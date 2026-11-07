pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct WindowTitle;

impl WindowTitle {
    pub fn create(screen: &str) -> String {
        let main = format!("ChuwitchWindow - ver {APP_VERSION}");
        if screen.is_empty() {
            main
        } else {
            format!("{screen} - {main}")
        }
    }
}
