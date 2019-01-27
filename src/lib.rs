//! # ravenlib
//! This powers [raven](https://git.sr.ht/~nicohman/raven), and provides an API for managing raven themes. Check raven for reasonably good example code.
#[macro_use]
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
extern crate dirs;
extern crate multipart;
extern crate reqwest;
extern crate tar;
#[macro_use]
extern crate error_chain;
#[macro_use]
extern crate log;
pub mod error;
/// Interactions with online instances of ThemeHub
pub mod ravenserver;
use std::fs::DirEntry;
/// Module for theme manipulation
pub mod themes;
/// Config module
pub mod config {
    use crate::themes::*;
    use dirs::home_dir;
    use error::*;
    use serde_json::value::Map;
    use std::{fs, fs::OpenOptions, io::Read, io::Write};
    /// Returns home directory as string
    pub fn get_home() -> String {
        return String::from(home_dir().unwrap().to_str().unwrap());
    }
    /// Default ravenserver host
    pub fn default_host() -> String {
        String::from("https://demenses.net")
    }
    /// Default screenshot url
    pub fn default_screen() -> String {
        String::new()
    }
    /// Default raven theme description
    pub fn default_desc() -> String {
        String::from("A raven theme.")
    }
    /// Config structure for holding all main config options
    #[derive(Serialize, Deserialize, Debug)]
    pub struct Config {
        pub monitors: i32,
        pub polybar: Vec<String>,
        pub menu_command: String,
        pub last: String,
        pub editing: String,
        #[serde(default = "default_host")]
        pub host: String,
    }
    impl Config {
        /// Default method for config file
        pub fn default() -> Config {
            Config {
                monitors: 1,
                polybar: vec!["main".to_string(), "other".to_string()],
                menu_command: "rofi -theme sidebar -mesg 'raven:' -p '> ' -dmenu".to_string(),
                last: "".to_string(),
                editing: "".to_string(),
                host: default_host(),
            }
        }
    }
    /// Check to see if there are themes still using the old format, and convert them if so.
    pub fn check_themes() -> Result<()> {
        let entries = get_themes()?;
        for entry in entries {
            info!("Checking theme {}", entry);
            if fs::metadata(get_home() + "/.config/raven/themes/" + &entry + "/theme").is_ok() {
                convert_theme(entry)?;
            }
        }
        Ok(())
    }
    /// Create base raven directories and config file(s)
    pub fn init() -> Result<()> {
        if fs::metadata(get_home() + "/.config/raven/config").is_err() {
            fs::create_dir(get_home() + "/.config/raven")?;
            fs::create_dir(get_home() + "/.config/raven/themes")?;
        } else {
            error!(
                    "The config file format has changed. Please check ~/.config/raven/config.json to reconfigure raven."
                );
        }
        info!("Creating config file");
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .open(get_home() + "/.config/raven/config.json")?;
        let default = serde_json::to_string(&Config::default())?;
        info!("Writing default config to file");
        file.write_all(default.as_bytes())?;
        info!("Correctly initialized base config and directory structure.");
        Ok(())
    }
    /// Checks to see if base config/directories need to be initialized
    pub fn check_init() -> bool {
        fs::metadata(get_home() + "/.config/raven").is_err()
            || fs::metadata(get_home() + "/.config/raven/config.json").is_err()
            || fs::metadata(get_home() + "/.config/raven/themes").is_err()
    }
    /// Updates and replaces the stored config with a new config
    pub fn up_config(conf: Config) -> Result<Config> {
        info!("Opening and writing to temp config file");
        OpenOptions::new()
            .create(true)
            .write(true)
            .open(get_home() + "/.config/raven/~config.json")?
            .write_all(serde_json::to_string(&conf)?.as_bytes())?;
        info!("Copying temp file to config file");
        fs::copy(
            get_home() + "/.config/raven/~config.json",
            get_home() + "/.config/raven/config.json",
        )?;
        info!("Removing temp file");
        fs::remove_file(get_home() + "/.config/raven/~config.json")?;
        Ok(conf)
    }
    /// Converts a theme from the old pipe-delineated format to the new json format
    pub fn convert_theme<N>(theme_name: N) -> Result<ThemeStore>
    where
        N: Into<String>,
    {
        let theme_name = theme_name.into();
        let mut theme = String::new();
        let otp = get_home() + "/.config/raven/themes/" + &theme_name + "/theme";
        fs::File::open(&otp)
            .expect("Couldn't read theme")
            .read_to_string(&mut theme)?;
        let options = theme
            .split('|')
            .map(|x| String::from(String::from(x).trim()))
            .filter(|x| x.len() > 0)
            .filter(|x| x != "|")
            .collect::<Vec<String>>();
        fs::remove_file(otp)?;
        let themes = ThemeStore {
            name: theme_name.clone(),
            enabled: Vec::new(),
            options: options.iter().map(|x| x.to_string()).collect(),
            screenshot: default_screen(),
            description: default_desc(),
            kv: Map::new(),
        };
        OpenOptions::new()
            .create(true)
            .write(true)
            .open(get_home() + "/.config/raven/themes/" + &theme_name + "/theme.json")?
            .write_all(serde_json::to_string(&themes)?.as_bytes())?;
        Ok(themes)
    }
    /// Retrieve config settings from file
    pub fn get_config() -> Result<Config> {
        let mut conf = String::new();
        info!("Opening and reading config file");
        fs::File::open(get_home() + "/.config/raven/config.json")?.read_to_string(&mut conf)?;
        info!("Parsing config file");
        Ok(serde_json::from_str(&conf)?)
    }
}
/// Ravend control
pub mod daemon {
    use error::*;
    use std::process::Child;
    use std::process::Command;
    /// Starts ravend
    pub fn start_daemon() -> Result<Child> {
        info!("Starting ravend");
        let child = Command::new("sh").arg("-c").arg("ravend").spawn()?;
        info!("Started cycle daemon.");
        Ok(child)
    }
    /// Stops ravend
    pub fn stop_daemon() -> Result<()> {
        info!("Starting pkill command");
        Command::new("pkill")
            .arg("-SIGKILL")
            .arg("ravend")
            .output()?;
        info!("Stopped cycle daemon.");
        Ok(())
    }
    /// Checks if the ravend daemon is running
    pub fn check_daemon() -> Result<bool> {
        info!("Starting ps command");
        let out = Command::new("ps").arg("aux").output()?;
        let form_out = String::from_utf8_lossy(&out.stdout);
        let line_num = form_out.lines().filter(|x| x.contains("ravend")).count();
        Ok(line_num > 0)
    }

}

/// Converts DirEntry into a fully processed file/directory name
pub fn proc_path(path: DirEntry) -> String {
    path.file_name().into_string().unwrap()
}
