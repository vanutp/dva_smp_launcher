use std::sync::Arc;
use std::thread;
use std::time::Duration;

use actix_web::{App, HttpResponse, HttpServer, Responder, web};
use tokio::io::AsyncWriteExt;
use tokio::sync::Notify;
use tokio::task;

slint::include_modules!();


fn main() {
    let ui = AppWindow::new().unwrap();

    // let (tx, rx) = mpsc::channel();
    thread::spawn({
        let ui_handle = ui.as_weak();
        move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    let got_response_flag = Arc::new(Notify::new());
                    let got_response_flag_setter = got_response_flag.clone();
                    let got_response_flag_getter = got_response_flag.clone();

                    let http_server = HttpServer::new(|| {
                        App::new()
                            .route("/hey", web::get().to(|| {
                                async {
                                    got_response_flag_setter.notify_one();
                                    HttpResponse::Ok().body("meow")
                                }
                            }))
                    })
                        .bind(("127.0.0.1", 8080))?
                        .run();

                    let server_handle = http_server.handle();

                    task::spawn(async move {
                        got_response_flag_getter.notified().await;
                        server_handle.stop(true);
                    });

                    http_server.await
                }).unwrap();
            println!("meow");

            ui_handle.upgrade_in_event_loop(|ui| {}).unwrap();
        }
    });

    ui.on_request_increase_value({
        let ui_handle = ui.as_weak();
        move || {
            let ui = ui_handle.unwrap();
            ui.set_counter(ui.get_counter() + 0.1);
        }
    });

    ui.run().unwrap();
}
