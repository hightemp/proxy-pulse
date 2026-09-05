use std::ffi::OsString;

const LOCAL_HOSTS: &str =
    "localhost,127.0.0.1,::1,[::1],ipc.localhost,tauri.localhost,asset.localhost";

fn proxy_exclusions(uppercase: Option<OsString>, lowercase: Option<OsString>) -> OsString {
    let mut exclusions = OsString::from(LOCAL_HOSTS);
    for existing in [uppercase, lowercase].into_iter().flatten() {
        if !existing.is_empty() {
            exclusions.push(",");
            exclusions.push(existing);
        }
    }
    exclusions
}

/// Configure only this process, before Tauri/WebKit or worker threads start.
pub fn configure_before_runtime() {
    let exclusions = proxy_exclusions(std::env::var_os("NO_PROXY"), std::env::var_os("no_proxy"));
    // Different native networking backends read different casings. Keep their
    // lists consistent and preserve the user's existing non-local exclusions.
    std::env::set_var("NO_PROXY", &exclusions);
    std::env::set_var("no_proxy", exclusions);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_ui_hosts_are_excluded_when_no_list_is_configured() {
        assert_eq!(proxy_exclusions(None, None), OsString::from(LOCAL_HOSTS));
    }

    #[test]
    fn both_existing_lists_are_preserved() {
        assert_eq!(
            proxy_exclusions(
                Some(".internal,10.0.0.0/8".into()),
                Some("other.example:8080".into())
            ),
            OsString::from(format!(
                "{LOCAL_HOSTS},.internal,10.0.0.0/8,other.example:8080"
            ))
        );
    }

    #[test]
    fn empty_and_wildcard_values_keep_their_meaning() {
        assert_eq!(
            proxy_exclusions(Some(OsString::new()), Some("*".into())),
            OsString::from(format!("{LOCAL_HOSTS},*"))
        );
    }

    #[cfg(unix)]
    #[test]
    fn non_unicode_environment_values_are_not_lossily_rewritten() {
        use std::os::unix::ffi::OsStringExt;
        let existing = OsString::from_vec(vec![b'x', 0xff]);
        let mut expected = LOCAL_HOSTS.as_bytes().to_vec();
        expected.extend_from_slice(&[b',', b'x', 0xff]);
        assert_eq!(proxy_exclusions(Some(existing), None).into_vec(), expected);
    }
}
