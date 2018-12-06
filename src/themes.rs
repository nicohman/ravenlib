use crate::config::*;
use error::*;
use proc_path;
use serde_json::value::{Map, Value};
use std::{
    env, fs, fs::DirEntry, fs::OpenOptions, io, io::Read, io::Write, os::unix::fs::OpenOptionsExt,
    process::Command,
};
/// Structure for holding theme info, stored in theme.json
#[derive(Serialize, Deserialize, Debug)]
pub struct ThemeStore {
    pub name: String,
    pub options: Vec<String>,
    pub enabled: Vec<String>,
    #[serde(default = "default_screen")]
    pub screenshot: String,
    #[serde(default = "default_desc")]
    pub description: String,
    #[serde(default)]
    pub kv: Map<String, Value>,
}
/// Structure that holds all methods and data for individual themes.
#[derive(Clone)]
pub struct Theme {
    pub name: String,
    pub options: Vec<ROption>,
    pub monitor: i32,
    pub enabled: Vec<String>,
    pub order: Vec<String>,
    pub kv: Map<String, Value>,
    pub screenshot: String,
    pub description: String,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
pub enum ROption {
    #[serde(rename = "poly")]
    Polybar,
    #[serde(rename = "wm")]
    OldI3,
    I3,
    Xres,
    #[serde(rename = "xres_m")]
    MergeXRes,
    Pywal,
    Wall,
    Ncmpcpp,
    Termite,
    Script,
    Bspwm,
    Rofi,
    Ranger,
    Lemonbar,
    Openbox,
    Dunst,
    VsCode,
    #[serde(rename = "st_subltheme")]
    OldSublTheme,
    #[serde(rename = "st_scs")]
    OldScs,
    #[serde(rename = "st_tmtheme")]
    OldTmTheme,
}
impl ROption {
    pub fn to_string(&self) -> String {
        return serde_json::to_string(self).unwrap();
    }
}
/// Methods for a loaded theme
impl Theme {
    /// Loads options held within theme.json key-value storage
    pub fn load_kv(&self) {
        for (k, v) in &self.kv {
            self.load_k(k.as_str(), v.as_str().unwrap()).unwrap();
        }
    }
    /// Loads a single key option
    pub fn load_k<N>(&self, k: N, v: N) -> Result<bool>
    where
        N: Into<String>,
    {
        let (k, v) = (k.into(), v.into());
        let mut ok = true;
        match k.as_str() {
            "st_tmtheme" => self.load_sublt("st_tmtheme", v.as_str())?,
            "st_scs" => self.load_sublt("st_scs", v.as_str())?,
            "st_subltheme" => self.load_sublt("st_subltheme", v.as_str())?,
            "vscode" => self.load_vscode(v.as_str())?,
            _ => {
                #[cfg(feature = "logging")]
                println!("Unrecognized key {}", k);
                ok = false;
                false
            }
        };
        #[cfg(feature = "logging")]
        println!("Loaded key option {}", k);
        Ok(ok)
    }
    /// Converts old single-string file options into key-value storage
    pub fn convert_single<N>(&self, name: N) -> Result<bool>
    where
        N: Into<String>,
    {
        let key = name.into();
        let mut value = String::new();
        fs::File::open(get_home() + "/.config/raven/themes/" + &self.name + "/" + &key)?
            .read_to_string(&mut value)?;
        let mut store = load_store(self.name.clone())?;
        store.kv.insert(
            key.clone(),
            serde_json::Value::String(value.clone().trim().to_string()),
        );
        store.options = store
            .options
            .iter()
            .filter(|x| x.as_str() != key.as_str())
            .map(|x| x.to_owned())
            .collect();
        up_theme(store)?;
        #[cfg(feature = "logging")]
        println!("Converted option {} to new key-value system", key);
        Ok(self.load_k(key, value)?)
    }
    /// Iterates through options and loads them with submethods
    pub fn load_all(&self) -> Result<()> {
        use crate::themes::ROption::*;
        let opt = &self.options;
        let mut i = 1;
        let len = opt.len();
        while i <= len {
            let ref option = opt[len - i];
            #[cfg(feature = "logging")]
            println!("Loading option {}", option.to_string());
            match option {
                Polybar => self.load_poly(self.monitor).unwrap(),
                OldI3 => self.load_i3(true).unwrap(),
                I3 => self.load_i3(false).unwrap(),
                Xres => self.load_xres(false).unwrap(),
                MergeXRes => self.load_xres(true).unwrap(),
                Pywal => self.load_pywal().unwrap(),
                Wall => self.load_wall().unwrap(),
                Ncmpcpp => {
                    self.load_ncm().unwrap();
                }
                Termite => self.load_termite().unwrap(),
                Script => self.load_script().unwrap(),
                Bspwm => self.load_bspwm().unwrap(),
                Rofi => self.load_rofi().unwrap(),
                Ranger => self.load_ranger().unwrap(),
                Lemonbar => self.load_lemon().unwrap(),
                Openbox => self.load_openbox().unwrap(),
                Dunst => self.load_dunst().unwrap(),
                OldTmTheme => {
                    self.convert_single("st_tmtheme").unwrap();
                },
                OldScs => {
                    self.convert_single("st_scs").unwrap();
                },
                OldSublTheme => {
                    self.convert_single("st_subltheme").unwrap();
                },
                VsCode => {
                    self.convert_single("vscode").unwrap();
                },
            };
            #[cfg(feature = "logging")]
            println!("Loaded option {}", option.to_string());
            i += 1;
        }
        self.load_kv();
        #[cfg(feature = "logging")]
        println!("Loaded all options for theme {}", self.name);
        Ok(())
    }
    /// Edits the value of a key in hjson files
    fn edit_hjson<N, S, T>(&self, file: N, pat: S, value: T) -> Result<()>
    where
        N: Into<String>,
        S: Into<String>,
        T: Into<String>,
    {
        let file = &file.into();
        let pat = &pat.into();
        let value = &value.into();
        let mut finals = String::new();
        if fs::metadata(file).is_ok() {
            let mut pre = String::new();
            fs::File::open(file)?.read_to_string(&mut pre)?;
            let mut patfound = false;
            for line in pre.lines() {
                if line.contains(pat) {
                    patfound = true;
                    if line.ends_with(",") {
                        finals = finals + "\n" + "    " + pat + "\"" + &value + "\","
                    } else {
                        finals = finals + "\n" + "    " + pat + "\"" + &value + "\""
                    }
                } else if line.ends_with("}") && !patfound {
                    finals =
                        finals + "," + "\n" + "    " + pat + "\"" + &value + "\"" + "\n" + line;
                } else {
                    finals = finals + "\n" + line;
                }
            }
            OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(file)?
                .write_all(finals.trim().as_bytes())?
        } else {
            finals = finals + "{\n    " + pat + "\"" + &value + "\"\n}";
            OpenOptions::new()
                .create(true)
                .write(true)
                .open(file)?
                .write_all(finals.as_bytes())?
        }
        Ok(())
    }
    pub fn load_rofi(&self) -> Result<()> {
        if fs::metadata(get_home() + "/.config/rofi").is_err() {
            fs::create_dir(get_home() + "/.config/rofi")?;
        }
        fs::copy(
            get_home() + "/.config/raven/themes/" + &self.name + "/rofi",
            get_home() + "/.config/rofi/theme.rasi",
        )?;
        Ok(())
    }
    pub fn load_pywal(&self) -> Result<()> {
        let arg = get_home() + "/.config/raven/themes/" + &self.name + "/pywal";
        Command::new("wal").arg("-n").arg("-i").arg(arg).output()?;
        Ok(())
    }
    pub fn load_script(&self) -> Result<()> {
        Command::new("sh")
            .arg("-c")
            .arg(get_home() + "/.config/raven/themes/" + &self.name + "/script")
            .output()?;
        Ok(())
    }

    pub fn load_openbox(&self) -> Result<()> {
        let mut base = String::new();
        if fs::metadata(get_home() + "/.config/raven/base_rc.xml").is_ok() {
            fs::File::open(get_home() + "/.config/raven/base_rc.xml")?.read_to_string(&mut base)?;
        }
        let mut rest = String::new();
        fs::File::open(get_home() + "/.config/raven/themes/" + &self.name + "/openbox")?
            .read_to_string(&mut rest)?;
        base.push_str(&rest);
        fs::remove_file(get_home() + "/.config/openbox/rc.xml")?;
        OpenOptions::new()
            .create(true)
            .write(true)
            .open(get_home() + "/.config/openbox/rc.xml")?
            .write_all(base.as_bytes())?;
        Command::new("openbox").arg("--reconfigure").output()?;
        Ok(())
    }
    pub fn load_ranger(&self) -> Result<()> {
        fs::copy(
            get_home() + "/.config/raven/themes/" + &self.name + "/ranger",
            get_home() + "/.config/ranger/rc.conf",
        )?;
        Ok(())
    }

    pub fn load_dunst(&self) -> Result<()> {
        let mut config = String::new();
        if fs::metadata(get_home() + "/.config/raven/base_dunst").is_ok() {
            fs::File::open(get_home() + "/.config/raven/base_dunst")?
                .read_to_string(&mut config)?;
        }
        let mut app = String::new();
        fs::File::open(get_home() + "/.config/raven/themes/" + &self.name + "/dunst")?
            .read_to_string(&mut app)?;
        config.push_str(&app);
        fs::remove_file(get_home() + "/.config/dunst/dunstrc")?;
        OpenOptions::new()
            .create(true)
            .write(true)
            .open(get_home() + "/.config/dunst/dunstrc")?
            .write_all(config.as_bytes())?;
        Command::new("dunst").spawn()?;
        Ok(())
    }
    pub fn load_vscode<N>(&self, value: N) -> Result<bool>
    where
        N: Into<String>,
    {
        let path1 = get_home() + "/.config/Code/User";
        let path2 = get_home() + "/.config/Code - OSS/User";
        if fs::metadata(&path1).is_err() && fs::metadata(&path2).is_err() {
            #[cfg(feature = "logging")]
            println!(
                "Couldn't find neither .config/Code nor .config/Code - OSS. Do you have VSCode installed? \
                Skipping."
            );
            return Ok(false);
        }
        let pattern = "\"workbench.colorTheme\": ";
        let value = value.into();
        if fs::metadata(&path1).is_ok() {
            self.edit_hjson(path1 + "/settings.json", pattern, value.as_str())?;
        }
        if fs::metadata(&path2).is_ok() {
            self.edit_hjson(path2 + "/settings.json", pattern, value)?;
        }
        Ok(true)
    }
    pub fn load_sublt<N>(&self, stype: N, value: N) -> Result<bool>
    where
        N: Into<String>,
    {
        let stype = &stype.into();
        let path = get_home() + "/.config/sublime-text-3/Packages/User";
        if fs::metadata(&path).is_err() {
            #[cfg(feature = "logging")]
            println!(
                "Couldn't find {}. Do you have sublime text 3 installed? \
                 Skipping.",
                &path
            );
            return Ok(false);
        }

        let mut value = value.into();
        if value.starts_with("sublt/") {
            value = value.trim_start_matches("sublt/").to_string();
            fs::copy(
                get_home() + "/.config/raven/themes/" + &self.name + "/sublt/" + &value,
                path.clone() + "/" + &value,
            )?;
        }

        let mut pattern = "";
        if stype == "st_tmtheme" || stype == "st_scs" {
            pattern = "\"color_scheme\": ";
        } else if stype == "st_subltheme" {
            pattern = "\"theme\": ";
        }
        self.edit_hjson(path + "/Preferences.sublime-settings", pattern, value)?;
        Ok(true)
    }

    pub fn load_ncm(&self) -> Result<bool> {
        if fs::metadata(get_home() + "/.config/ncmpcpp").is_ok() {
            fs::copy(
                get_home() + "/.config/raven/themes/" + &self.name + "/ncmpcpp",
                get_home() + "/.config/ncmpcpp/config",
            )?;
        } else if fs::metadata(get_home() + "/.ncmpcpp").is_ok() {
            fs::copy(
                get_home() + "/.config/raven/themes/" + &self.name + "/ncmpcpp",
                get_home() + "/.ncmpcpp/config",
            )?;
        } else {
            #[cfg(feature = "logging")]
            println!(
                "Couldn't detect a ncmpcpp config directory in ~/.config/ncmppcp or ~/.ncmpcpp."
            );
            return Ok(false);
        }
        Ok(true)
    }
    pub fn load_bspwm(&self) -> Result<()> {
        let mut config = String::new();
        if fs::metadata(get_home() + "/.config/raven/base_bspwm").is_ok() {
            fs::File::open(get_home() + "/.config/raven/base_bspwm")?
                .read_to_string(&mut config)?;
        }
        let mut app = String::new();
        fs::File::open(get_home() + "/.config/raven/themes/" + &self.name + "/bspwm")?
            .read_to_string(&mut app)?;
        config.push_str(&app);
        fs::remove_file(get_home() + "/.config/bspwm/bspwmrc")?;
        OpenOptions::new()
            .create(true)
            .write(true)
            .mode(0o744)
            .open(get_home() + "/.config/bspwm/bspwmrc")?
            .write_all(config.as_bytes())?;
        Command::new("sh")
            .arg("-c")
            .arg(get_home() + "/.config/bspwm/bspwmrc")
            .output()?;
        Ok(())
    }
    pub fn load_i3(&self, isw: bool) -> Result<()> {
        let mut config = String::new();
        if fs::metadata(get_home() + "/.config/raven/base_i3").is_ok() {
            fs::File::open(get_home() + "/.config/raven/base_i3")?.read_to_string(&mut config)?;
        }
        let mut app = String::new();
        if isw {
            fs::File::open(get_home() + "/.config/raven/themes/" + &self.name + "/wm")
                .unwrap()
                .read_to_string(&mut app)
                .unwrap();
        } else {
            fs::File::open(get_home() + "/.config/raven/themes/" + &self.name + "/i3")?
                .read_to_string(&mut app)?;
        }
        config.push_str(&app);
        if fs::metadata(get_home() + "/.config/i3").is_err() {
            fs::create_dir(get_home() + "/.config/i3")?;
        }
        if fs::metadata(get_home() + "/.config/i3/config").is_ok() {
            fs::remove_file(get_home() + "/.config/i3/config")?;
        }
        OpenOptions::new()
            .create(true)
            .write(true)
            .open(get_home() + "/.config/i3/config")?
            .write_all(config.as_bytes())?;
        Command::new("i3-msg").arg("reload").output()?;
        Ok(())
    }
    pub fn load_termite(&self) -> Result<()> {
        fs::copy(
            get_home() + "/.config/raven/themes/" + &self.name + "/termite",
            get_home() + "/.config/termite/config",
        )?;
        Command::new("pkill")
            .arg("-SIGUSR1")
            .arg("termite")
            .output()?;
        Ok(())
    }
    pub fn load_poly(&self, monitor: i32) -> Result<()> {
        for number in 0..monitor {
            Command::new("sh")
                .arg("-c")
                .arg(
                    String::from("polybar --config=")
                        + &get_home()
                        + "/.config/raven/themes/"
                        + &self.name
                        + "/poly "
                        + &self.order[number as usize]
                        + " > /dev/null 2> /dev/null",
                )
                .spawn()?;
        }
        Ok(())
    }
    fn load_lemon(&self) -> Result<()> {
        Command::new("sh")
            .arg(get_home() + "/.config/raven/themes/" + &self.name + "/lemonbar")
            .spawn()?;
        Ok(())
    }
    fn load_wall(&self) -> Result<()> {
        Command::new("feh")
            .arg("--bg-scale")
            .arg(get_home() + "/.config/raven/themes/" + &self.name + "/wall")
            .output()?;
        Ok(())
    }
    fn load_xres(&self, merge: bool) -> Result<()> {
        let mut xres = Command::new("xrdb");
        let mut name = String::from("xres");
        if merge {
            name.push_str("_m");
            xres.arg("-merge");
        }
        xres.arg(get_home() + "/.config/raven/themes/" + &self.name + "/" + &name)
            .output()?;
        Ok(())
    }
}

/// Changes the theme that is currently being edited
pub fn edit<N>(theme_name: N) -> Result<String>
where
    N: Into<String>,
{
    let theme_name = theme_name.into();
    if fs::metadata(get_home() + "/.config/raven/themes/" + &theme_name).is_ok() {
        let mut conf = get_config()?;
        conf.editing = theme_name.to_string();
        up_config(conf)?;
        #[cfg(feature = "logging")]
        println!("You are now editing the theme {}", &theme_name);
        Ok(theme_name)
    } else {
        Err(ErrorKind::InvalidThemeName(theme_name).into())
    }
}
/// Clears possible remnants of old themes
pub fn clear_prev() -> Result<()> {
    Command::new("pkill").arg("polybar").output()?;
    Command::new("pkill").arg("lemonbar").output()?;
    Command::new("pkill").arg("dunst").output()?;
    Ok(())
}
/// Deletes theme from registry
pub fn del_theme<N>(theme_name: N) -> Result<()>
where
    N: Into<String>,
{
    fs::remove_dir_all(get_home() + "/.config/raven/themes/" + &theme_name.into())?;
    Ok(())
}
/// Loads last loaded theme from string of last theme's name
pub fn refresh_theme<N>(last: N) -> Result<()>
where
    N: Into<String>,
{
    let last = last.into();
    if last.chars().count() > 0 {
        run_theme(&load_theme(last.trim())?)?;
        Ok(())
    } else {
        #[cfg(feature = "logging")]
        println!("No last theme saved. Cannot refresh.");
        Err(ErrorKind::InvalidThemeName(last).into())
    }
}
/// Create new theme directory and 'theme' file
pub fn new_theme<N>(theme_name: N) -> Result<()>
where
    N: Into<String>,
{
    let theme_name = theme_name.into();
    fs::create_dir(get_home() + "/.config/raven/themes/" + &theme_name)?;
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .open(get_home() + "/.config/raven/themes/" + &theme_name + "/theme.json")?;
    let stdef = ThemeStore {
        name: theme_name.clone(),
        options: vec![],
        enabled: vec![],
        screenshot: default_screen(),
        description: default_desc(),
        kv: Map::new(),
    };
    let st = serde_json::to_string(&stdef)?;
    file.write_all(st.as_bytes())?;
    edit(theme_name)?;
    Ok(())
}
/// Add an option to a theme
pub fn add_to_theme<N>(theme_name: N, option: N, path: N) -> Result<()>
where
    N: Into<String>,
{
    let (theme_name, option, path) = (theme_name.into(), option.into(), path.into());
    let cur_theme = load_theme(theme_name.as_str())?;
    let cur_st = load_store(theme_name.as_str())?;
    let opts = cur_theme.options.iter().map(|x| x.to_string()).collect();
    let mut new_themes = ThemeStore {
        name: theme_name.clone(),
        options: opts,
        enabled: cur_theme.enabled,
        screenshot: cur_st.screenshot,
        description: cur_st.description,
        kv: Map::new(),
    };
    let mut already_used = false;
    for opt in &new_themes.options {
        if opt == &option {
            already_used = true;
        }
    }
    if !already_used {
        new_themes.options.push(option.clone());
        up_theme(new_themes)?;
    }
    let mut totpath = env::current_dir()?;
    totpath.push(path);
    fs::copy(
        totpath,
        get_home() + "/.config/raven/themes/" + &theme_name + "/" + &option,
    )?;
    Ok(())
}
/// Remove an option from a theme
pub fn rm_from_theme<N>(theme_name: N, option: N) -> Result<()>
where
    N: Into<String>,
{
    let (theme_name, option) = (theme_name.into(), option.into());
    let cur_theme = load_theme(theme_name.as_str())?;
    let cur_st = load_store(theme_name.as_str())?;
    let opts = cur_theme.options.iter().map(|x| x.to_string()).collect();
    let mut new_themes = ThemeStore {
        name: theme_name.clone(),
        options: opts,
        enabled: cur_theme.enabled,
        screenshot: cur_st.screenshot,
        description: cur_st.description,
        kv: Map::new(),
    };
    let mut found = false;
    let mut i = 0;
    while i < new_themes.options.len() {
        if &new_themes.options[i] == &option {
            #[cfg(feature = "logging")]
            println!("Found option {}", option);
            found = true;
            new_themes.options.remove(i);
        }
        i += 1;
    }
    if found {
        up_theme(new_themes)?;
        Ok(())
    } else {
        #[cfg(feature = "logging")]
        println!("Couldn't find option {}", option);
        Err(ErrorKind::InvalidThemeName(theme_name).into())
    }
}
/// Run/refresh a loaded Theme
pub fn run_theme(new_theme: &Theme) -> Result<()> {
    clear_prev()?;
    new_theme.load_all()?;
    // Updates the 'last loaded theme' information for later use by raven refresh
    let mut conf = get_config()?;
    conf.last = new_theme.name.clone();
    up_config(conf)?;
    Ok(())
}
/// Get all themes
pub fn get_themes() -> Result<Vec<String>> {
    Ok(fs::read_dir(get_home() + "/.config/raven/themes")?
        .collect::<Vec<io::Result<DirEntry>>>()
        .into_iter()
        .map(|x| proc_path(x.unwrap()))
        .collect::<Vec<String>>())
}
/// Changes a key-value option
pub fn key_value<N, S, T>(key: N, value: S, theme: T) -> Result<()>
where
    N: Into<String>,
    S: Into<String>,
    T: Into<String>,
{
    let mut store = load_store(theme.into())?;
    store
        .kv
        .insert(key.into(), serde_json::Value::String(value.into()));
    up_theme(store)?;
    Ok(())
}
/// Load in data for a specific theme
pub fn load_theme<N>(theme_name: N) -> Result<Theme>
where
    N: Into<String>,
{
    let theme_name = theme_name.into();

    let conf = get_config()?;
    let ent_res = fs::read_dir(get_home() + "/.config/raven/themes/" + &theme_name);
    if ent_res.is_ok() {
        #[cfg(feature = "logging")]
        println!("Found theme {}", theme_name);
        if fs::metadata(get_home() + "/.config/raven/themes/" + &theme_name + "/theme.json").is_ok()
        {
            let theme_info = load_store(theme_name.as_str()).unwrap();
            let opts: Vec<ROption> = theme_info
                .options
                .iter()
                .filter_map(|x| {
                    let res = serde_json::from_value(json!(x));
                    res.ok()
                })
                .map(|x: Option<ROption>| x.unwrap())
                .collect();
            let new_theme = Theme {
                name: theme_name,
                options: opts,
                monitor: conf.monitors,
                enabled: theme_info.enabled,
                order: conf.polybar,
                kv: theme_info.kv,
                screenshot: theme_info.screenshot,
                description: theme_info.description,
            };
            Ok(new_theme)
        } else {
            Err(ErrorKind::InvalidThemeName(theme_name).into())
        }
    } else {
        #[cfg(feature = "logging")]
        println!("Theme does not exist.");
        Err(ErrorKind::InvalidThemeName(theme_name).into())
    }
}
/// Loads all themes
pub fn load_themes() -> Result<Vec<Theme>> {
    Ok(get_themes()?
        .iter()
        .map(|x| load_theme(x.as_str()))
        .filter(|x| x.is_ok())
        .map(|x| x.unwrap())
        .collect::<Vec<Theme>>())
}
