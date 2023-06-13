mod ely_by;

use std::thread;

use tokio::runtime;

slint::include_modules!();


fn main() {
    let ui = AppWindow::new().unwrap();

    thread::spawn({
        let rt = runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Failed to create asynchronous runtime");

        let ui_handle = ui.as_weak();
        move || rt.block_on(async {
            let set_page = |page: i32| {
                ui_handle.upgrade_in_event_loop(move |ui| {
                    ui.set_page(page);
                }).unwrap();
            };

            set_page(0);
            let token = ely_by::authorize().await;
            println!("{:?}", token);
            set_page(-1);
        })
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
