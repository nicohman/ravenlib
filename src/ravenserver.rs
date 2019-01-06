use config::*;
use dirs::home_dir;
use reqwest;
use serde_json;
use std::{
    fs,
    fs::{File, OpenOptions},
    io,
    io::{Read, Write},
    error::Error
};
use error::*;
use tar::{Archive, Builder};
fn get_home() -> String {
    return String::from(home_dir().unwrap().to_str().unwrap());
}
/// Information on the currently logged in user
#[derive(Serialize, Deserialize, Debug)]
pub struct UserInfo {
    /// The user's username
    name: String,
    /// The login token to use when making requests
    token: String,
}
/// A response containing a theme's metadata
#[derive(Serialize, Deserialize, Debug)]
pub struct MetaRes {
    screen: String,
    description: String,
}
/// Loads in info on the currently logged in user
pub fn load_info() -> Result<UserInfo> {
    if fs::metadata(get_home() + "/.config/raven/ravenserver.json").is_ok() {
        let mut info = String::new();
        File::open(get_home() + "/.config/raven/ravenserver.json")?
            .read_to_string(&mut info)?;
        return Ok(serde_json::from_str(&info)?);
    } else {
        Err(ErrorKind::Server(RavenServerErrorKind::NotLoggedIn.into()).into())
    }
}
/// Exports a theme to a tar file, returning the file's name
pub fn export<N>(theme_name: N, tmp: bool) -> Result<String>
where
    N: Into<String>,
{
    let theme_name = theme_name.into();
    if fs::metadata(get_home() + "/.config/raven/themes/" + &theme_name).is_ok() {
        let mut tname = String::new();
        if tmp {
            tname = tname + "/tmp/";
        }
        tname = tname + &theme_name.to_string() + ".tar";
        let tb = File::create(&tname)?;
        let mut b = Builder::new(tb);
        b.append_dir_all(
            theme_name.to_string(),
            get_home() + "/.config/raven/themes/" + &theme_name,
        )?;
        b.into_inner()?;
        #[cfg(feature = "logging")]
        println!("Wrote theme to {}", tname);
        Ok(tname)
    } else {
        #[cfg(feature = "logging")]
        println!("Theme does not exist");
        Err(ErrorKind::InvalidThemeName(theme_name).into())
    }
}
/// Imports a theme from a tar file
pub fn import<N>(file_name: N) -> Result<()>
where
    N: Into<String>,
{
    let fname: String = file_name.into();
    let fd = File::open(fname)?;
    let mut arch = Archive::new(fd);
    arch.unpack(get_home() + "/.config/raven/themes/")?;
    #[cfg(feature = "logging")]
    println!("Imported theme.");
    Ok(())
}
/// Replaces and updates a stored userinfo file
fn up_info(inf: UserInfo) -> Result<()>{
    let winfpath = get_home() + "/.config/raven/~ravenserver.json";
    let infpath = get_home() + "/.config/raven/ravenserver.json";
    OpenOptions::new()
        .create(true)
        .write(true)
        .open(&winfpath)?
        .write_all(serde_json::to_string(&inf)?.as_bytes())
        .expect("Couldn't write to user info file");
    fs::copy(&winfpath, &infpath)?;
    fs::remove_file(&winfpath)?;
    Ok(())
}
/// Logs a user out by deleting the userinfo file
pub fn logout() -> Result<()> {
    fs::remove_file(get_home() + "/.config/raven/ravenserver.json")?;
    #[cfg(feature = "logging")]
    println!("Successfully logged you out");
    Ok(())
}
/// Gets the configured ThemeHub host
pub fn get_host() -> Result<String> {
    let conf = get_config()?;
    Ok(conf.host)
}
/// Makes a call to delete the currently logged in user. Requires password confirmation
pub fn delete_user<N>(pass: N) -> Result<()>
where
    N: Into<String>,
{
    let info = load_info()?;
    let client = reqwest::Client::new();
    let res = client
        .post(
            &(get_host()?
                + "/themes/users/delete/"
                + &info.name
                + "?token="
                + &info.token
                + "&pass="
                + &pass.into()),
        )
        .send()?;

        if res.status().is_success() {
            #[cfg(feature = "logging")]
            println!("Successfully deleted user and all owned themes. Logging out");
            logout()?;
            Ok(())
        } else {
            if res.status() == reqwest::StatusCode::FORBIDDEN {
                #[cfg(feature = "logging")]
                println!("You are trying to delete a user you are not. Bad!");
                Err(ErrorKind::Server(RavenServerErrorKind::PermissionDenied.into()).into())
            } else if res.status() == reqwest::StatusCode::UNAUTHORIZED {
                #[cfg(feature = "logging")]
                println!("You're trying to delete a user w/o providing authentication credentials");
                Err(ErrorKind::Server(RavenServerErrorKind::NotLoggedIn.into()).into())
            } else if res.status() == reqwest::StatusCode::NOT_FOUND {
                #[cfg(feature = "logging")]
                println!("You're trying to delete a user that doesn't exist");
                Err(ErrorKind::Server(RavenServerErrorKind::DoesNotExist(info.name).into()).into())
            } else {
                #[cfg(feature = "logging")]
                println!("Server error. Code {:?}", res.status());
                Err(ErrorKind::Server(RavenServerErrorKind::ServerError(res.status()).into()).into())
            }
        }
}
/// Creates a user with the given name and password. Pass and pass2 must match, or function will return false.
pub fn create_user<N>(name: N, pass: N, pass2: N) -> Result<bool>
where
    N: Into<String>,
{
    let (name, pass, pass2): (String, String, String) = (name.into(), pass.into(), pass2.into());
    if pass == pass2 {
        let client = reqwest::Client::new();
        let res = client
            .post(&(get_host()? + "/themes/user/create?name=" + &name + "&pass=" + &pass))
            .send()?;
            if res.status().is_success() {
                #[cfg(feature = "logging")]
                println!("Successfully created user. Sign in with `raven login [name] [password]`");
                Ok(true)
            } else {
                if res.status() == reqwest::StatusCode::FORBIDDEN {
                    #[cfg(feature = "logging")]
                    println!("User already created. Pick a different name!");
                    Err(ErrorKind::Server(RavenServerErrorKind::PermissionDenied.into()).into())
                } else if res.status() == reqwest::StatusCode::PAYLOAD_TOO_LARGE {
                    #[cfg(feature = "logging")]
                    println!(
                            "Either your username or password was too long. The limit is 20 characters for username, and 100 for password."
                        );
                        Err(ErrorKind::Server(RavenServerErrorKind::TooLarge.into()).into())
                } else {
                    #[cfg(feature = "logging")]
                    println!("Server error. Code {:?}", res.status());
                    Err(ErrorKind::Server(RavenServerErrorKind::ServerError(res.status()).into()).into())
                }
            }

    } else {
        #[cfg(feature = "logging")]
        println!("Passwords need to match");
        return Ok(false);
    }
}
/// Uploads a theme of the given name. Returns true if the theme was created, or false if it was just updated.
pub fn upload_theme<N>(name: N) -> Result<bool>
where
    N: Into<String>,
{
    let name = name.into();
    let info = load_info()?;
    if fs::metadata(get_home() + "/.config/raven/themes/" + &name).is_ok() {
        let tname = export(name.as_str(), true)?;
            let form = reqwest::multipart::Form::new()
                .file("fileupload", &tname)?;
            let res = reqwest::Client::new()
                .post(&(get_host()? + "/themes/upload?name=" + &name + "&token=" + &info.token))
                .multipart(form)
                .send()?;
                if res.status().is_success() {
                    let mut up = false;
                    if res.status() == reqwest::StatusCode::CREATED {
                        #[cfg(feature = "logging")]
                        println!("Theme successfully uploaded.");
                        up = true;
                    }
                    let theme_st = load_store(name.as_str())?;
                    if theme_st.screenshot != default_screen() {
                        pub_metadata(name.as_str(), "screen".into(), &theme_st.screenshot)?;
                    }
                    pub_metadata(name, "description".into(), theme_st.description)?;
                    fs::remove_file(tname)?;
                    Ok(up)
                } else {
                    if res.status() == reqwest::StatusCode::FORBIDDEN {
                        #[cfg(feature = "logging")]
                        println!("That theme already exists, and you are not its owner.");
                        Err(ErrorKind::Server(RavenServerErrorKind::PermissionDenied.into()).into())
                    } else {
                        #[cfg(feature = "logging")]
                        println!("Server error. Code {:?}", res.status());
                        Err(ErrorKind::Server(RavenServerErrorKind::ServerError(res.status()).into()).into())
                    }
                }
    } else {
        #[cfg(feature = "logging")]
        println!("That theme does not exist");
        Err(ErrorKind::InvalidThemeName(name).into())
    }
}
/// Sends a request to get the metadata of a theme
pub fn get_metadata<N>(name: N) -> Result<MetaRes>
where
    N: Into<String>,
{
    let name = name.into();
    let client = reqwest::Client::new();
    let mut res = client
        .get(&(get_host()? + "/themes/meta/" + &name))
        .send()?;
        if res.status().is_success() {
            let meta: MetaRes = res.json()?;
            Ok(meta)
        } else {
            if res.status() == reqwest::StatusCode::NOT_FOUND {
                Err(ErrorKind::Server(RavenServerErrorKind::DoesNotExist(name).into()).into())
            } else {
                Err(ErrorKind::Server(RavenServerErrorKind::ServerError(res.status()).into()).into())
            }
        }

}
/// Publishes the metadata of a theme online, with the type and value given
pub fn pub_metadata<N>(name: N, typem: N, value: N) -> Result<()>
where
    N: Into<String>,
{
    let info = load_info()?;
    let name = name.into();
    let client = reqwest::Client::new();
    let res = client
        .post(
            &(get_host()?
                + "/themes/meta/"
                + &name
                + "?typem="
                + &typem.into()
                + "&value="
                + &value.into()
                + "&token="
                + &info.token),
        )
        .send()?;
        if res.status().is_success() {
            #[cfg(feature = "logging")]
            println!("Successfully updated theme metadata");
            Ok(())
        } else {
            if res.status() == reqwest::StatusCode::NOT_FOUND {
                #[cfg(feature = "logging")]
                println!("That theme hasn't been published");
                Err(ErrorKind::Server(RavenServerErrorKind::DoesNotExist(name).into()).into())
            } else if res.status() == reqwest::StatusCode::FORBIDDEN {
                #[cfg(feature = "logging")]
                println!("Can't edit the metadata of a theme that isn't yours");
                Err(ErrorKind::Server(RavenServerErrorKind::PermissionDenied.into()).into())
            } else if res.status() == reqwest::StatusCode::PRECONDITION_FAILED {
                #[cfg(feature = "logging")]
                println!("That isn't a valid metadata type");
                Err(ErrorKind::Server(RavenServerErrorKind::PreConditionFailed("metadata type".to_string()).into()).into())
            } else if res.status() == reqwest::StatusCode::PAYLOAD_TOO_LARGE {
                #[cfg(feature = "logging")]
                println!(
                        "Your description or screenshot url was more than 200 characters long. Please shorten it."
                    );
                Err(ErrorKind::Server(RavenServerErrorKind::TooLarge.into()).into())
            } else {
                #[cfg(feature = "logging")]
                println!("Server error. Code {:?}", res.status());
                Err(ErrorKind::Server(RavenServerErrorKind::ServerError(res.status()).into()).into())
            }
        }
}
/// Deletes a theme from the online repo
pub fn unpublish_theme<N>(name: N) -> Result<()>
where
    N: Into<String>,
{
    let name = name.into();
    let info = load_info()?;
    let client = reqwest::Client::new();
    let res = client
        .post(&(get_host()? + "/themes/delete/" + &name + "?token=" + &info.token))
        .send()?;
        if res.status().is_success() {
            #[cfg(feature = "logging")]
            println!("Successfully unpublished theme");
            Ok(())
        } else {
            if res.status() == reqwest::StatusCode::NOT_FOUND {
                #[cfg(feature = "logging")]
                println!("Can't unpublish a nonexistent theme");
                Err(ErrorKind::Server(RavenServerErrorKind::DoesNotExist(name).into()).into())
            } else if res.status() == reqwest::StatusCode::FORBIDDEN {
                #[cfg(feature = "logging")]
                println!("Can't unpublish a theme that isn't yours");
                Err(ErrorKind::Server(RavenServerErrorKind::PermissionDenied.into()).into())
            } else if res.status() == reqwest::StatusCode::UNAUTHORIZED {
                #[cfg(feature = "logging")]
                println!("Did not provide a valid login token");
                Err(ErrorKind::Server(RavenServerErrorKind::PermissionDenied.into()).into())
            } else {
                #[cfg(feature = "logging")]
                println!("Server error. Code {:?}", res.status());
                Err(ErrorKind::Server(RavenServerErrorKind::ServerError(res.status()).into()).into())
            }
        }
}
/// Prints out a warning when installing a theme
pub fn install_warning(esp: bool) {
    println!(
            "Warning: When you install themes from the online repo, there is some danger. Please evaluate the theme files before loading the theme, and if you find any malicious theme, please report it on the theme's page at {} and it will be removed.",
            get_host().unwrap()
        );
    if esp {
        println!(
                "This theme should be scrutinized more carefully as it includes a bash script which will be run automatically."
            );
    }
    println!("Thank you for helping keep the repo clean!");
}
/// Checks if the /tmp directory exists
pub fn check_tmp() -> bool {
    fs::metadata("/tmp").is_ok()
}
/// Downloads a theme from online. Force ignores all warning prompts.
pub fn download_theme<N>(name: N, force: bool) -> Result<bool>
where
    N: Into<String>,
{
    let name = name.into();
    let mut tname = String::new();
    if check_tmp() {
        tname = tname + "/tmp/";
    }
    tname = tname + &name + ".tar";
    println!("{}", tname);
    let client = reqwest::Client::new();
    let mut res = client.get(&(get_host()? + "/themes/repo/" + &name)).send()?;
        if res.status().is_success() {
            let mut file = OpenOptions::new()
                .create(true)
                .write(true)
                .open(&tname)?;
            res.copy_to(&mut file)?;
            println!("Downloaded theme.");
            if res.status() == reqwest::StatusCode::ALREADY_REPORTED && !force {
                print!(
                        "This theme has recently been reported, and has not been approved by an admin. It is not advisable to install this theme. Are you sure you would like to continue? (y/n)"
                    );
                io::stdout().flush()?;
                let mut r = String::new();
                io::stdin().read_line(&mut r)?;
                if r.trim() == "y" {
                    println!(
                            "Continuing. Please look carefully at the theme files in ~/.config/raven/themes/{} before loading this theme.",
                            name
                        );
                    import(tname.as_str())?;
                    #[cfg(feature = "logging")]
                    println!("Imported theme. Removing archive.");
                    fs::remove_file(&tname)?;
                    #[cfg(feature = "logging")]
                    println!("Downloading metadata.");
                    let meta = get_metadata(name.as_str())?;
                    let mut st = load_store(name.as_str())?;
                    st.screenshot = meta.screen;
                    st.description = meta.description;
                    up_theme(st)?;
                    if fs::metadata(get_home() + "/.config/raven/themes/" + &name + "/script")
                        .is_ok()
                        || fs::metadata(get_home() + "/.config/raven/themes/" + &name + "/lemonbar")
                            .is_ok()
                    {
                        if !force {
                            install_warning(true);
                        }
                    } else {
                        if !force {
                            install_warning(false);
                        }
                    }
                    Ok(true)
                } else {
                    #[cfg(feature = "logging")]
                    println!("Removing downloaded archive.");
                    fs::remove_file(&tname)?;
                    Ok(false)
                }
            } else {
                if res.status() == reqwest::StatusCode::ALREADY_REPORTED {
                    #[cfg(feature = "logging")]
                    print!(
                            "This theme has recently been reported, and has not been approved by an admin. It is not advisable to install this theme. Continuing because of --force."
                        );
                }
                import(tname.as_str())?;
                #[cfg(feature = "logging")]
                println!("Imported theme. Removing archive.");
                fs::remove_file(tname)?;
                #[cfg(feature = "logging")]
                println!("Downloading metadata.");
                let meta = get_metadata(name.as_str())?;
                let mut st = load_store(name.as_str())?;
                st.screenshot = meta.screen;
                st.description = meta.description;
                up_theme(st)?;
                if fs::metadata(get_home() + "/.config/raven/themes/" + &name + "/script").is_ok()
                    || fs::metadata(get_home() + "/.config/raven/themes/" + &name + "/lemonbar")
                        .is_ok()
                {
                    if !force {
                        install_warning(true);
                    }
                } else {
                    if !force {
                        install_warning(false);
                    }
                }
                Ok(true)
            }
        } else {
            if res.status() == reqwest::StatusCode::NOT_FOUND {
                #[cfg(feature = "logging")]
                println!("Theme has not been uploaded");
                Err(ErrorKind::Server(RavenServerErrorKind::DoesNotExist(name).into()).into())
            } else {
                #[cfg(feature = "logging")]
                println!("Server error. Code {:?}", res.status());
                Err(ErrorKind::Server(RavenServerErrorKind::ServerError(res.status()).into()).into())
            }
        }
}
/// Logs a user in and writes userinfo file to disk
pub fn login_user<N>(name: N, pass: N) -> Result<()>
where
    N: Into<String>,
{
    let client = reqwest::Client::new();
    let mut res = client
        .get(&(get_host()? + "/themes/user/login?name=" + &name.into() + "&pass=" + &pass.into()))
        .send()?;
        if res.status().is_success() {
            #[cfg(feature = "logging")]
            println!("Successfully signed in. Writing login info to disk.");
            let info = res.json()?;
            up_info(info)?;
            Ok(())
        } else {
            if res.status() == reqwest::StatusCode::FORBIDDEN {
                #[cfg(feature = "logging")]
                println!("Wrong login info. Try again!");
                Err(ErrorKind::Server(RavenServerErrorKind::PermissionDenied.into()).into())
            } else {
                #[cfg(feature = "logging")]
                println!("Server error. Code {:?}", res.status());
                Err(ErrorKind::Server(RavenServerErrorKind::ServerError(res.status()).into()).into())
            }
        }
}
