use crate::models::{CardwireClient, CardwireManagerProxy};
use rfd::MessageDialog;
use zbus::Connection;

async fn get_connection() -> Connection {
    loop {
        match Connection::system().await {
            Ok(conn) => return conn,
            Err(_) => {
                let dialog = MessageDialog::new()
                    .set_level(rfd::MessageLevel::Warning)
                    .set_title("Cardwire DBus Connection")
                    .set_description("Failed to connect to the DBus system bus.")
                    .set_buttons(rfd::MessageButtons::OkCancelCustom(
                        "Retry".into(),
                        "Quit".into(),
                    ));

                if dialog.show() == rfd::MessageDialogResult::Custom("Quit".to_string()) {
                    std::process::exit(1);
                }
            }
        }
    }
}

pub async fn get_client() -> CardwireClient {
    let conn = get_connection().await;
    return loop {
        match CardwireManagerProxy::new(&conn).await {
            Ok(p) => {
                if p.status().await.is_ok() {
                    break CardwireClient::new(conn);
                }
            }
            Err(_) => {}
        }

        let dialog = MessageDialog::new()
            .set_level(rfd::MessageLevel::Warning)
            .set_title("Cardwire DBus Service")
            .set_description("Could not access the cardwire DBus service. Is it running?")
            .set_buttons(rfd::MessageButtons::OkCancelCustom(
                "Retry".into(),
                "Quit".into(),
            ));

        if dialog.show() == rfd::MessageDialogResult::Custom("Quit".to_string()) {
            std::process::exit(1);
        }
    };
}
