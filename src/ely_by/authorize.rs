use std::error::Error;
use std::fmt::{Display, Formatter};
use std::sync::Mutex;

use actix_web::{web, App, HttpResponse, HttpServer};
use anyhow::bail;
use clone_macro::clone;
use serde::Deserialize;
use tokio::select;
use tokio::sync::oneshot;

const CLIENT_ID: &str = "dvasmp1";
const CLIENT_SECRET: &str = "ICLLyGfzhLCyHhAiH5kmcL4QOt7N7RNrQbDkrUE1kB5GOu_EPo503iz3nsiZ34mq";
const LISTEN_PORT: u16 = 18741;
const REDIRECT_URI: &str = "http://127.0.0.1:18741/callback";

#[derive(Deserialize)]
struct OauthCallbackData {
    code: String,
}

#[derive(Deserialize)]
struct ElyByOauthTokenResponse {
    access_token: String,
    token_type: String,
}

#[derive(Debug, Clone)]
struct ElyByAuthError;

impl Display for ElyByAuthError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Authentication error")
    }
}

impl Error for ElyByAuthError {}

pub async fn authorize() -> Result<String, anyhow::Error> {
    let url = format!(
        "https://account.ely.by/oauth2/v1\
            ?client_id={}\
            &redirect_uri={}\
            &response_type=code\
            &scope=account_info minecraft_server_session\
            &prompt=select_account",
        CLIENT_ID, REDIRECT_URI
    );
    webbrowser::open(url.as_str())?;

    let (sender, receiver) = oneshot::channel();

    let sender = web::Data::new(Mutex::new(Some(sender)));

    let http_server = HttpServer::new(clone!([sender], move || {
        App::new().app_data(sender.clone()).route(
            "/callback",
            web::get().to(
                move |req: web::Query<OauthCallbackData>,
                      sender: web::Data<Mutex<Option<oneshot::Sender<_>>>>| async move {
                    let result = request_token(&req.code).await;
                    let is_success = result.is_ok();

                    sender
                        .lock()
                        .expect("Failed to acquire result sender")
                        .take()
                        .map(|sender| sender.send(result));

                    if is_success {
                        HttpResponse::Found()
                            .insert_header((
                                "Location",
                                "https://account.ely.by/oauth2/code/success?appName=DVA SMP",
                            ))
                            .finish()
                    } else {
                        HttpResponse::InternalServerError().body(
                            "аааааа ошибка стоп 000000 пришло время переустанавливать шиндовс",
                        )
                    }
                },
            ),
        )
    }))
    .bind(("127.0.0.1", LISTEN_PORT))?
    .run();

    let server_handle = http_server.handle();

    select! {
        result = http_server => {
            server_handle.stop(false).await;
            result?;

            bail!("Server was stopped too early")
        }
        result = receiver => {
            result?
        }
    }
}

async fn request_token(code: &str) -> Result<String, anyhow::Error> {
    let token_response: ElyByOauthTokenResponse = reqwest::Client::new()
        .post("https://account.ely.by/api/oauth2/v1/token")
        .form(&[
            ("client_id", CLIENT_ID),
            ("client_secret", CLIENT_SECRET),
            ("redirect_uri", REDIRECT_URI),
            ("grant_type", "authorization_code"),
            ("code", code),
        ])
        .send()
        .await?
        .json()
        .await?;

    assert_eq!(token_response.token_type, "Bearer");

    Ok(token_response.access_token)
}
