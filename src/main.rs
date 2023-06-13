mod ely_by;

use std::thread;

slint::include_modules!();


fn main() {
    let ui = AppWindow::new().unwrap();

    // let (tx, rx) = mpsc::channel();
    thread::spawn({
        let ui_handle = ui.as_weak();
        move || {
            let set_page = |page: i32| {
                ui_handle.upgrade_in_event_loop(move |ui| {
                    ui.set_page(page);
                }).unwrap();
            };

            set_page(0);
            let token = ely_by::authorize();
            println!("{}", token);
            set_page(-1);
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
