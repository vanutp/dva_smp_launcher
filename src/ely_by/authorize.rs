use std::error::Error;
use std::fmt::{Display, Formatter};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use actix_web::{App, HttpResponse, HttpServer, web};
use clone_macro::clone;
use serde::Deserialize;
use tokio::sync::Notify;
use tokio::task;

const CLIENT_ID: &str = "dvasmp1";
const CLIENT_SECRET: &str = "ICLLyGfzhLCyHhAiH5kmcL4QOt7N7RNrQbDkrUE1kB5GOu_EPo503iz3nsiZ34mq";
const LISTEN_PORT: u16 = 18741;
const REDIRECT_URI: &str = "http://127.0.0.1:18741/callback";

#[derive(Deserialize)]
struct ElyByOauthCallbackData {
    code: String,
}

#[derive(Deserialize)]
struct ElyByOauthTokenResponse {
    access_token: String,
    token_type: String,
}

struct ActixAppState {
    access_token: Mutex<Option<String>>,
    is_error: Mutex<bool>,
}

#[derive(Debug, Clone)]
struct ElyByAuthError;

impl Display for ElyByAuthError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Authentication error")
    }
}

impl Error for ElyByAuthError {}

pub fn authorize() -> Result<String, anyhow::Error> {
    let url = format!("https://account.ely.by/oauth2/v1\
            ?client_id={}\
            &redirect_uri={}\
            &response_type=code\
            &scope=account_info minecraft_server_session\
            &prompt=select_account", CLIENT_ID, REDIRECT_URI);
    webbrowser::open(url.as_str())?;

    let app_data = web::Data::new(ActixAppState {
        access_token: Mutex::new(None),
        is_error: Mutex::new(false),
    });
    let outer_app_data = app_data.clone();

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(async {
            let got_response_flag = Arc::new(Notify::new());
            let http_server = HttpServer::new(clone!([got_response_flag], move || {
                App::new()
                .app_data(app_data.clone())
                .route(
                    "/callback",
                    web::get().to(clone!([got_response_flag], move |req: web::Query<ElyByOauthCallbackData>, data: web::Data<ActixAppState>| clone!([got_response_flag], async move {
                        let return_error = || {
                            let mut is_error = data.is_error.lock().unwrap();
                            *is_error = true;
                            HttpResponse::InternalServerError().body("аааааа ошибка стоп 000000 пришло время переустанавливать шиндовс")
                        };

                        let client = reqwest::Client::new();
                        let token_response = client
                            .post("https://account.ely.by/api/oauth2/v1/token")
                            .form(&[
                                ("client_id", CLIENT_ID),
                                ("client_secret", CLIENT_SECRET),
                                ("redirect_uri", REDIRECT_URI),
                                ("grant_type", "authorization_code"),
                                ("code", &req.code),
                            ])
                            .send()
                            .await;
                        if token_response.is_err() {
                            return return_error();
                        }
                        let token_response = token_response.unwrap().json::<ElyByOauthTokenResponse>().await;
                        if token_response.is_err() {
                            return return_error();
                        }
                        let token_response = token_response.unwrap();
                        assert_eq!(token_response.token_type, "Bearer");
                        let mut access_token = data.access_token.lock().unwrap();
                        *access_token = Some(token_response.access_token);
                        got_response_flag.notify_one();
                        HttpResponse::Found()
                            .insert_header(("Location", "https://account.ely.by/oauth2/code/success?appName=DVA SMP"))
                            .finish()
                    }))),
                )
            }))
                .bind(("127.0.0.1", LISTEN_PORT))?
                .run();

            let server_handle = http_server.handle();

            task::spawn(async move {
                got_response_flag.notified().await;
                tokio::time::sleep(Duration::from_millis(100)).await;
                server_handle.stop(false).await;
            });

            http_server.await
        })?;

    if *outer_app_data.is_error.lock()? {
        return Err(anyhow::Error::new(ElyByAuthError));
    }

    let res = outer_app_data.access_token.lock()?.as_ref()?.to_string();
    Ok(res)
}
