use config::*;
use dirs::home_dir;
use error::*;
use reqwest;
use serde_json;
use std::{
    error::Error,
    fs,
    fs::{File, OpenOptions},
    io,
    io::{Read, Write},
};
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
        info!("Opening and reading user info file");
        File::open(get_home() + "/.config/raven/ravenserver.json")?.read_to_string(&mut info)?;
        return Ok(serde_json::from_str(&info)?);
    } else {
        error!("Could not load user info");
        Err(ErrorKind::Server(RavenServerErrorKind::NotLoggedIn.into()).into())
    }
}
/// Exports a theme to a tar file, returning the file's name
pub fn export<N>(theme_name: N, tmp: bool) -> Result<String>
where
    N: Into<String>,
{
    info!("Exporting theme");
    let theme_name = theme_name.into();
    if fs::metadata(get_home() + "/.config/raven/themes/" + &theme_name).is_ok() {
        let mut tname = String::new();
        if tmp {
            info!("Using temp directory /tmp");
            tname = tname + "/tmp/";
        }
        tname = tname + &theme_name.to_string() + ".tar";
        info!("Creating output file");
        let tb = File::create(&tname)?;
        let mut b = Builder::new(tb);
        info!("Importing theme into tar builder");
        b.append_dir_all(
            theme_name.to_string(),
            get_home() + "/.config/raven/themes/" + &theme_name,
        )?;
        b.into_inner()?;
        info!("Wrote theme to {}", tname);
        Ok(tname)
    } else {
        error!("Theme does not exist");
        Err(ErrorKind::InvalidThemeName(theme_name).into())
    }
}
/// Imports a theme from a tar file
pub fn import<N>(file_name: N) -> Result<()>
where
    N: Into<String>,
{
    let fname: String = file_name.into();
    info!("Opening theme file");
    let fd = File::open(fname)?;
    info!("Converting opened file to archive reader");
    let mut arch = Archive::new(fd);
    info!("Unpacking archive");
    arch.unpack(get_home() + "/.config/raven/themes/")?;
    info!("Imported theme.");
    Ok(())
}
/// Replaces and updates a stored userinfo file
fn up_info(inf: UserInfo) -> Result<()> {
    info!("Updating stored userinfo");
    let winfpath = get_home() + "/.config/raven/~ravenserver.json";
    let infpath = get_home() + "/.config/raven/ravenserver.json";
    info!("Opening temp file and writing to it");
    OpenOptions::new()
        .create(true)
        .write(true)
        .open(&winfpath)?
        .write_all(serde_json::to_string(&inf)?.as_bytes())
        .expect("Couldn't write to user info file");
    info!("Copying temp file to final file");
    fs::copy(&winfpath, &infpath)?;
    info!("Removing temp file");
    fs::remove_file(&winfpath)?;
    Ok(())
}
/// Logs a user out by deleting the userinfo file
pub fn logout() -> Result<()> {
    info!("Removing ravenserver config file");
    fs::remove_file(get_home() + "/.config/raven/ravenserver.json")?;
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
    info!("Making delete post request");
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
        println!("Successfully deleted user and all owned themes. Logging out");
        logout()?;
        Ok(())
    } else {
        if res.status() == reqwest::StatusCode::FORBIDDEN {
            error!("You are trying to delete a user you are not. Bad!");
            Err(ErrorKind::Server(RavenServerErrorKind::PermissionDenied.into()).into())
        } else if res.status() == reqwest::StatusCode::UNAUTHORIZED {
            error!("You're trying to delete a user w/o providing authentication credentials");
            Err(ErrorKind::Server(RavenServerErrorKind::NotLoggedIn.into()).into())
        } else if res.status() == reqwest::StatusCode::NOT_FOUND {
            error!("You're trying to delete a user that doesn't exist");
            Err(ErrorKind::Server(RavenServerErrorKind::DoesNotExist(info.name).into()).into())
        } else {
            error!("Server error. Code {:?}", res.status());
            Err(ErrorKind::Server(RavenServerErrorKind::ServerError(res.status()).into()).into())
        }
    }
}
/// Creates a user with the given name and password. Pass and pass2 must match, or function will return false.
pub fn create_user(
    name: impl Into<String>,
    pass: impl Into<String>,
    pass2: impl Into<String>,
) -> Result<bool> {
    let (name, pass, pass2): (String, String, String) = (name.into(), pass.into(), pass2.into());
    if pass == pass2 {
        let client = reqwest::Client::new();
        info!("Making create post request");
        let res = client
            .post(&(get_host()? + "/themes/user/create?name=" + &name + "&pass=" + &pass))
            .send()?;
        if res.status().is_success() {
            println!("Successfully created user. Sign in with `raven login [name] [password]`");
            Ok(true)
        } else {
            if res.status() == reqwest::StatusCode::FORBIDDEN {
                error!("User already created. Pick a different name!");
                Err(ErrorKind::Server(RavenServerErrorKind::PermissionDenied.into()).into())
            } else if res.status() == reqwest::StatusCode::PAYLOAD_TOO_LARGE {
                error!(
                            "Either your username or password was too long. The limit is 20 characters for username, and 100 for password."
                        );
                Err(ErrorKind::Server(RavenServerErrorKind::TooLarge.into()).into())
            } else {
                error!("Server error. Code {:?}", res.status());
                Err(
                    ErrorKind::Server(RavenServerErrorKind::ServerError(res.status()).into())
                        .into(),
                )
            }
        }
    } else {
        error!("Passwords need to match");
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
        info!("Creating multipart upload form");
        let form = reqwest::multipart::Form::new().file("fileupload", &tname)?;
        info!("Making upload post request");
        let res = reqwest::Client::new()
            .post(&(get_host()? + "/themes/upload?name=" + &name + "&token=" + &info.token))
            .multipart(form)
            .send()?;
        if res.status().is_success() {
            let mut up = false;
            if res.status() == reqwest::StatusCode::CREATED {
                info!("Theme successfully uploaded.");
                up = true;
            }
            let theme_st = load_store(name.as_str())?;
            if theme_st.screenshot != default_screen() {
                info!("Publishing screenshot metadata");
                pub_metadata(name.as_str(), "screen".into(), &theme_st.screenshot)?;
            }
            info!("Publishing description metadata");
            pub_metadata(name, "description".into(), theme_st.description)?;
            fs::remove_file(tname)?;
            Ok(up)
        } else {
            if res.status() == reqwest::StatusCode::FORBIDDEN {
                error!("That theme already exists, and you are not its owner.");
                Err(ErrorKind::Server(RavenServerErrorKind::PermissionDenied.into()).into())
            } else {
                error!("Server error. Code {:?}", res.status());
                Err(
                    ErrorKind::Server(RavenServerErrorKind::ServerError(res.status()).into())
                        .into(),
                )
            }
        }
    } else {
        error!("That theme does not exist");
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
    info!("Making metadata get request");
    let mut res = client
        .get(&(get_host()? + "/themes/meta/" + &name))
        .send()?;
    if res.status().is_success() {
        let meta: MetaRes = res.json()?;
        Ok(meta)
    } else {
        if res.status() == reqwest::StatusCode::NOT_FOUND {
            error!("Metadata does not exist on server");
            Err(ErrorKind::Server(RavenServerErrorKind::DoesNotExist(name).into()).into())
        } else {
            error!("Server error: {:?}", res.status());
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
    info!("Making metadata publish request");
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
        info!("Successfully updated theme metadata");
        Ok(())
    } else {
        if res.status() == reqwest::StatusCode::NOT_FOUND {
            error!("That theme hasn't been published");
            Err(ErrorKind::Server(RavenServerErrorKind::DoesNotExist(name).into()).into())
        } else if res.status() == reqwest::StatusCode::FORBIDDEN {
            error!("Can't edit the metadata of a theme that isn't yours");
            Err(ErrorKind::Server(RavenServerErrorKind::PermissionDenied.into()).into())
        } else if res.status() == reqwest::StatusCode::PRECONDITION_FAILED {
            error!("That isn't a valid metadata type");
            Err(ErrorKind::Server(
                RavenServerErrorKind::PreConditionFailed("metadata type".to_string()).into(),
            )
            .into())
        } else if res.status() == reqwest::StatusCode::PAYLOAD_TOO_LARGE {
            error!(
                        "Your description or screenshot url was more than 200 characters long. Please shorten it."
                    );
            Err(ErrorKind::Server(RavenServerErrorKind::TooLarge.into()).into())
        } else {
            error!("Server error. Code {:?}", res.status());
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
        info!("Successfully unpublished theme");
        Ok(())
    } else {
        if res.status() == reqwest::StatusCode::NOT_FOUND {
            error!("Can't unpublish a nonexistent theme");
            Err(ErrorKind::Server(RavenServerErrorKind::DoesNotExist(name).into()).into())
        } else if res.status() == reqwest::StatusCode::FORBIDDEN {
            error!("Can't unpublish a theme that isn't yours");
            Err(ErrorKind::Server(RavenServerErrorKind::PermissionDenied.into()).into())
        } else if res.status() == reqwest::StatusCode::UNAUTHORIZED {
            error!("Did not provide a valid login token");
            Err(ErrorKind::Server(RavenServerErrorKind::PermissionDenied.into()).into())
        } else {
            error!("Server error. Code {:?}", res.status());
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
    info!("Downloading theme {}", name);
    let mut tname = String::new();
    if check_tmp() {
        tname = tname + "/tmp/";
    }
    tname = tname + &name + ".tar";
    let client = reqwest::Client::new();
    info!("Requesting theme from server");
    let mut res = client
        .get(&(get_host()? + "/themes/repo/" + &name))
        .send()?;
    if res.status().is_success() {
        info!("Opening file {}", tname);
        let mut file = OpenOptions::new().create(true).write(true).open(&tname)?;
        info!("Copying response to file");
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
                info!("Importing downloaded theme");
                import(tname.as_str())?;
                info!("Imported theme. Removing archive.");
                fs::remove_file(&tname)?;
                info!("Downloading metadata.");
                let meta = get_metadata(name.as_str())?;
                let mut st = load_store(name.as_str())?;
                st.screenshot = meta.screen;
                st.description = meta.description;
                info!("Updating local theme store");
                up_theme(st)?;
                if fs::metadata(get_home() + "/.config/raven/themes/" + &name + "/script").is_ok()
                    || fs::metadata(get_home() + "/.config/raven/themes/" + &name + "/lemonbar")
                        .is_ok()
                {
                    info!("Theme has script or lemonbar. Printing higher warning");
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
                info!("Removing downloaded archive.");
                fs::remove_file(&tname)?;
                Ok(false)
            }
        } else {
            if res.status() == reqwest::StatusCode::ALREADY_REPORTED {
                print!(
                            "This theme has recently been reported, and has not been approved by an admin. It is not advisable to install this theme. Continuing because of --force."
                        );
            }
            info!("Importing theme");
            import(tname.as_str())?;
            info!("Imported theme. Removing archive.");
            fs::remove_file(tname)?;
            info!("Downloading metadata.");
            let meta = get_metadata(name.as_str())?;
            let mut st = load_store(name.as_str())?;
            st.screenshot = meta.screen;
            st.description = meta.description;
            info!("Updating local theme store");
            up_theme(st)?;
            if fs::metadata(get_home() + "/.config/raven/themes/" + &name + "/script").is_ok()
                || fs::metadata(get_home() + "/.config/raven/themes/" + &name + "/lemonbar").is_ok()
            {
                info!("Theme has script or lemonbar. Printing higher warning");

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
            error!("Theme has not been uploaded");
            Err(ErrorKind::Server(RavenServerErrorKind::DoesNotExist(name).into()).into())
        } else {
            error!("Server error. Code {:?}", res.status());
            Err(ErrorKind::Server(RavenServerErrorKind::ServerError(res.status()).into()).into())
        }
    }
}
/// Logs a user in and writes userinfo file to disk
pub fn login_user(name: impl Into<String>, pass: impl Into<String>) -> Result<()> {
    let client = reqwest::Client::new();
    info!("Making login request");
    let mut res = client
        .get(&(get_host()? + "/themes/user/login?name=" + &name.into() + "&pass=" + &pass.into()))
        .send()?;
    if res.status().is_success() {
        info!("Successfully signed in. Writing login info to disk.");
        let info = res.json()?;
        up_info(info)?;
        Ok(())
    } else {
        if res.status() == reqwest::StatusCode::FORBIDDEN {
            error!("Wrong login info. Try again!");
            Err(ErrorKind::Server(RavenServerErrorKind::PermissionDenied.into()).into())
        } else {
            error!("Server error. Code {:?}", res.status());
            Err(ErrorKind::Server(RavenServerErrorKind::ServerError(res.status()).into()).into())
        }
    }
}
