use async_trait::async_trait;
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use oauth2::reqwest::async_http_client;
use oauth2::{AuthType, AuthUrl, AuthorizationCode, ClientId, CsrfToken, PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, RequestTokenError, Scope, TokenResponse, TokenUrl};
use reqwest::Client;
use serde::Deserialize;
use shared::utils::{BoxError, BoxResult};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use minecraft_msa_auth::MinecraftAuthorizationFlow;
use oauth2::basic::BasicErrorResponseType;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::time::sleep;

use crate::lang::LangMessage;
use crate::message_provider::MessageProvider;

use super::base::{AuthProvider, UserInfo};

const MSA_AUTHORIZE_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize";
const MSA_TOKEN_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";

#[derive(thiserror::Error, Debug)]
pub enum AuthError {
    #[error("Request error")]
    RequestError,
    #[error("Timeout during authentication")]
    AuthTimeout,
}

pub struct MicrosoftAuthProvider {
    client_id: String,
}

#[derive(Deserialize)]
struct AuthQuery {
    code: AuthorizationCode,
    state: CsrfToken,
}

#[derive(Deserialize)]
struct MinecraftProfileResponse {
    id: String,
    name: String,
}

#[derive(Clone)]
pub struct TokioExecutor;

impl<F> hyper::rt::Executor<F> for TokioExecutor
where
    F: std::future::Future + Send + 'static,
    F::Output: Send + 'static,
{
    fn execute(&self, fut: F) {
        tokio::task::spawn(fut);
    }
}

enum TokenResult {
    Token(String),
    InvalidState,
    InvalidCode,
    Error(BoxError),
}

async fn handle_request(
    client: oauth2::basic::BasicClient,
    pkce_code_verifier: PkceCodeVerifier,
    csrf_state: CsrfToken,
    req: Request<hyper::body::Incoming>,
    token_tx: Arc<mpsc::UnboundedSender<TokenResult>>,
) -> BoxResult<Response<Full<Bytes>>> {
    let query = req.uri().query().ok_or("Missing query string")?;
    let auth_query: AuthQuery = serde_urlencoded::from_str(query)?;

    if auth_query.state.secret() != csrf_state.secret() {
        token_tx.send(TokenResult::InvalidState).unwrap();
        return Ok(
            Response::builder()
                .status(400)
                .body(Full::new(Bytes::from("Invalid state")))?
        );
    }

    let token_response = client
        .exchange_code(auth_query.code)
        .set_pkce_verifier(pkce_code_verifier)
        .request_async(async_http_client).await;

    let token_result = match token_response {
        Ok(token_response) =>
            TokenResult::Token(token_response.access_token().secret().to_string()),

        Err(e) => match &e {
            RequestTokenError::ServerResponse(resp) => {
                if *resp.error() == BasicErrorResponseType::InvalidGrant {
                    TokenResult::InvalidCode
                } else {
                    TokenResult::Error(Box::new(e))
                }
            }
            _ => TokenResult::Error(Box::new(e)),
        }
    };

    let response = match &token_result {
        TokenResult::Token(_) => Response::builder()
            .status(200)
            .body(Full::new(Bytes::from("Success!!!")))?,

        TokenResult::InvalidCode => Response::builder()
            .status(400)
            .body(Full::new(Bytes::from("Invalid code")))?,

        TokenResult::InvalidState => Response::builder()
            .status(400)
            .body(Full::new(Bytes::from("Invalid state")))?,

        TokenResult::Error(_) => Response::builder()
            .status(500)
            .body(Full::new(Bytes::from("Internal server error")))?,
    };

    token_tx.send(token_result).unwrap();

    Ok(response)
}

async fn get_ms_token(
    client_id: String,
    message_provider: Arc<dyn MessageProvider>
) -> BoxResult<String> {
    let addr = SocketAddr::from(([127, 0, 0, 1], 0));
    let listener = TcpListener::bind(addr).await?;
    let redirect_uri = format!("http://localhost:{}/", listener.local_addr()?.port());

    let oauth_client = oauth2::basic::BasicClient::new(
        ClientId::new(client_id),
        None,
        AuthUrl::new(MSA_AUTHORIZE_URL.to_string())?,
        Some(TokenUrl::new(MSA_TOKEN_URL.to_string())?),
    )
        .set_auth_type(AuthType::RequestBody)
        .set_redirect_uri(RedirectUrl::new(redirect_uri)?);
    let (pkce_code_challenge, pkce_code_verifier) = PkceCodeChallenge::new_random_sha256();
    let (authorize_url, csrf_state) = oauth_client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("XboxLive.signin offline_access".to_string()))
        .set_pkce_challenge(pkce_code_challenge)
        .url();

    let _ = open::that(&authorize_url.to_string());
    message_provider.set_message(LangMessage::AuthMessage {
        url: authorize_url.to_string()
    });

    let mut http = http1::Builder::new();
    http.keep_alive(false);

    loop {
        let stream;
        tokio::select! {
                _ = sleep(Duration::from_secs(60 * 5)) => {
                    return Err(Box::new(AuthError::AuthTimeout));
                }

                st = listener.accept() => {
                    stream = st?.0;
                }
            }

        let io = TokioIo::new(stream);

        let (token_tx, mut token_rx) = mpsc::unbounded_channel();
        let token_tx = Arc::new(token_tx);

        http.serve_connection(
            io,
            service_fn(|req: Request<hyper::body::Incoming>| {
                let token_tx = token_tx.clone();
                handle_request(
                    oauth_client.clone(),
                    PkceCodeVerifier::new(pkce_code_verifier.secret().clone()),
                    csrf_state.clone(),
                    req,
                    token_tx,
                )
            }),
        )
            .await?;

        if let Some(token) = token_rx.recv().await {
            match token {
                TokenResult::Token(token) => return Ok(token),
                TokenResult::InvalidCode | TokenResult::InvalidState => {
                    continue;
                }
                TokenResult::Error(e) => return Err(e),
            }
        } else {
            return Err(Box::new(AuthError::RequestError));
        }
    }
}

impl MicrosoftAuthProvider {
    pub fn new(msa_client_id: &str) -> Self {
        MicrosoftAuthProvider {
            client_id: msa_client_id.to_string(),
        }
    }
}

#[async_trait]
impl AuthProvider for MicrosoftAuthProvider {
    async fn authenticate(&self, message_provider: Arc<dyn MessageProvider>) -> BoxResult<String> {
        let ms_token = get_ms_token(self.client_id.clone(), message_provider).await?;
        message_provider.clear();
        let mc_flow = MinecraftAuthorizationFlow::new(Client::new());
        let mc_token = mc_flow.exchange_microsoft_token(ms_token)
            .await?
            .access_token()
            .clone()
            .into_inner();

        Ok(mc_token)
    }

    async fn get_user_info(&self, token: &str) -> BoxResult<UserInfo> {
        let client = Client::new();
        let resp = client
            .get("https://api.minecraftservices.com/minecraft/profile")
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await?
            .error_for_status()?
            .json::<MinecraftProfileResponse>()
            .await?;

        Ok(UserInfo {
            uuid: resp.id,
            username: resp.name,
        })
    }

    fn get_auth_url(&self) -> Option<String> {
        None
    }

    fn get_name(&self) -> String {
        "Microsoft".to_string()
    }
}
