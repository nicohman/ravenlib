error_chain! {
    types {
        Error, ErrorKind, ResultExt, Result;
    }
    links {
        Server(RavenServerError, RavenServerErrorKind);
    }
    foreign_links {
        Fmt(::std::fmt::Error);
        Io(::std::io::Error);
        Network(::reqwest::Error);
        Parse(::serde_json::error::Error);
    }

    // Define additional `ErrorKind` variants. The syntax here is
    // the same as `quick_error!`, but the `from()` and `cause()`
    // syntax is not supported.
    errors {
        InvalidThemeName(t: String) {
            description("invalid theme name")
            display("invalid theme name: '{}'", t)
        }
    }
}
error_chain! {
    types {
        RavenServerError, RavenServerErrorKind, RavenServerResultExt, RavenServerResult;
    }
    foreign_links {
        Parse(::serde_json::error::Error);
        Network(::reqwest::Error);
    }
    errors {
        PermissionDenied {
            description("inadequate permissions")
            display("not allowed to perform this operation")
        }
        NotLoggedIn {
            description("not logged in")
            display("no login info stored")
        }
        DoesNotExist(t: String) {
            description("the requested resource does not exist")
            display("{} does not exist", t)
        }
        ServerError(code: ::reqwest::StatusCode) {
            description("the server encountered an error")
            display("the server encountered an error. code: {:?}", code)
        }
        TooLarge {
            description("payload was too large")
            display("a payload was sent that was too large")
            //heh
        }
        PreConditionFailed(t: String) {
            description("failed precondition for request")
            display("{} was failed", t)
        }
    }
}
