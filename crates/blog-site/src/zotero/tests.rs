//! Profile, download and launch boundary tests; never launch a real client.

use super::*;
use std::io::Write;
use std::net::TcpListener;
use std::sync::atomic::{AtomicU32, Ordering};

struct Fixture {
    root: PathBuf,
    zotero: Zotero,
}

impl Fixture {
    fn new() -> Self {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let root = std::env::temp_dir().join(format!(
            "zotero-helper-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join(".tools")).unwrap();
        let zotero = Zotero::new(&root).unwrap();
        Fixture {
            root: zotero.root.clone(),
            zotero,
        }
    }

    fn app(&self) -> PathBuf {
        let app = self
            .zotero
            .directory
            .join("client/Zotero_win-x64/zotero.exe");
        fs::create_dir_all(app.parent().unwrap()).unwrap();
        fs::write(&app, b"fixture executable; must not launch").unwrap();
        app
    }

    fn archive(&self, members: &[(&str, &[u8])]) -> PathBuf {
        fs::create_dir_all(&self.zotero.directory).unwrap();
        let path = self
            .zotero
            .directory
            .join(format!("Zotero-{VERSION}_win-x64.zip"));
        let mut writer = zip::ZipWriter::new(fs::File::create(&path).unwrap());
        for (name, content) in members {
            writer
                .start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(content).unwrap();
        }
        writer.finish().unwrap();
        path
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[derive(Default)]
struct Fake {
    running: bool,
    server_id: Option<&'static str>,
    status_calls: usize,
    setup_calls: usize,
    spawned: Vec<(Vec<OsString>, bool)>,
    app: PathBuf,
}

impl Launcher for Fake {
    fn status(&mut self, port: u16) -> Map<String, Value> {
        self.status_calls += 1;
        let mut state = Map::new();
        state.insert("running".into(), json!(self.running));
        if let Some(id) = self.server_id {
            state.insert("serverID".into(), json!(id));
        }
        state.insert("port".into(), json!(port));
        state
    }

    fn setup(&mut self, _: &Zotero) -> Result<PathBuf> {
        self.setup_calls += 1;
        Ok(self.app.clone())
    }

    fn spawn(&mut self, _: &Zotero, command: &[OsString], hidden: bool) -> Result<u32> {
        self.spawned.push((command.to_vec(), hidden));
        Ok(123)
    }
}

#[test]
fn private_path_rejects_outside_and_normalizes_inside() {
    let f = Fixture::new();
    assert!(f.zotero.private_path(&f.root.join("site/library")).is_err());
    assert_eq!(
        f.zotero
            .private_path(&f.root.join(".tools/one/../two"))
            .unwrap(),
        f.root.join(".tools").join("two")
    );
}

#[test]
fn profiles_named_differently_do_not_share_a_library() {
    let f = Fixture::new();
    let first = f
        .zotero
        .prepare_profile(&f.root.join(".tools/tests/first"), 23129)
        .unwrap();
    let second = f
        .zotero
        .prepare_profile(&f.root.join(".tools/tests/second"), 23130)
        .unwrap();
    let quoted = |path: PathBuf| python_json(&json!(path.display().to_string()));
    assert!(fs::read_to_string(first.join("user.js"))
        .unwrap()
        .contains(&quoted(first.parent().unwrap().join("first-library"))));
    assert!(fs::read_to_string(second.join("user.js"))
        .unwrap()
        .contains(&quoted(second.parent().unwrap().join("second-library"))));
    let normal = f
        .zotero
        .prepare_profile(&f.zotero.directory.join("profile"), 23119)
        .unwrap();
    let text = fs::read_to_string(normal.join("user.js")).unwrap();
    assert!(text.contains(&quoted(f.zotero.directory.join("library"))));
    assert!(text.starts_with(
        "user_pref(\"extensions.zotero.httpServer.enabled\", true);\nuser_pref(\"extensions.zotero.httpServer.localAPI.enabled\", true);\nuser_pref(\"extensions.zotero.httpServer.port\", 23119);\nuser_pref(\"extensions.zotero.useDataDir\", true);\n"
    ));
    assert!(text.ends_with("user_pref(\"extensions.zotero.firstRun2\", false);\n"));
}

#[test]
fn profile_symlink_cannot_redirect_writes() {
    let f = Fixture::new();
    let profile = f.zotero.directory.join("profile");
    fs::create_dir_all(&profile).unwrap();
    let outside = f.root.join("outside.js");
    fs::write(&outside, "preserve").unwrap();
    #[cfg(windows)]
    let linked = std::os::windows::fs::symlink_file(&outside, profile.join("user.js"));
    #[cfg(unix)]
    let linked = std::os::unix::fs::symlink(&outside, profile.join("user.js"));
    if linked.is_err() {
        return; // Windows symlink privilege unavailable.
    }
    assert!(f.zotero.prepare_profile(&profile, 23119).is_err());
    assert_eq!(fs::read_to_string(&outside).unwrap(), "preserve");
}

#[cfg(windows)]
#[test]
fn profile_junction_cannot_redirect_library_writes() {
    let f = Fixture::new();
    fs::create_dir_all(&f.zotero.directory).unwrap();
    let alternate = f.root.join("outside-profile");
    fs::create_dir_all(&alternate).unwrap();
    let redirected = f.zotero.directory.join("redirected-profile");
    let created = Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&redirected)
        .arg(&alternate)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    if !created.is_ok_and(|s| s.success()) {
        return; // This filesystem does not permit junction creation.
    }
    assert!(f.zotero.prepare_profile(&redirected, 23119).is_err());
    assert!(!alternate.join("user.js").exists());
}

#[test]
fn zip_rejects_traversal_absolute_ads_and_windows_normalization() {
    let f = Fixture::new();
    let client = f.zotero.directory.join("client");
    fs::create_dir_all(&client).unwrap();
    for name in [
        "../escape.exe",
        "/absolute.exe",
        "C:/absolute.exe",
        "a\\..\\escape.exe",
        "a/name.exe:stream",
        "a/../escape.exe",
        "a/NUL.txt",
        "a/COM1",
        "a/space /x.exe",
        "a/dot./x.exe",
        "./zotero.exe",
        "",
        "a/b\0c",
    ] {
        assert!(
            f.zotero.check_zip_member(&client, name, None).is_err(),
            "accepted {name:?}"
        );
    }
    let accepted = f
        .zotero
        .check_zip_member(&client, "Zotero_win-x64/app/omni.ja", None)
        .unwrap();
    assert_eq!(
        accepted,
        client.join("Zotero_win-x64").join("app").join("omni.ja")
    );
    assert!(f
        .zotero
        .check_zip_member(&client, "Zotero_win-x64/app/", None)
        .is_ok());
    assert!(f
        .zotero
        .check_zip_member(&client, "Zotero_win-x64/COM10", None)
        .is_ok());
}

#[test]
fn zip_rejects_symlinks() {
    let f = Fixture::new();
    let client = f.zotero.directory.join("client");
    fs::create_dir_all(&client).unwrap();
    let error = f
        .zotero
        .check_zip_member(&client, "Zotero_win-x64/link", Some(0o120777))
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "Links are not accepted in the Zotero ZIP"
    );
    assert!(f
        .zotero
        .check_zip_member(&client, "Zotero_win-x64/file", Some(0o100644))
        .is_ok());
}

#[test]
fn extraction_validates_all_members_before_writing() {
    let f = Fixture::new();
    let archive = f.archive(&[
        ("Zotero_win-x64/zotero.exe", b"fixture"),
        ("../escape", b"unsafe"),
    ]);
    let client = f.zotero.directory.join("client");
    fs::create_dir_all(&client).unwrap();
    assert!(f.zotero.extract(&archive, &client).is_err());
    assert!(!client.join("Zotero_win-x64/zotero.exe").exists());
}

#[test]
fn extraction_keeps_the_official_layout() {
    let f = Fixture::new();
    let archive = f.archive(&[
        ("Zotero_win-x64/zotero.exe", b"fixture"),
        ("Zotero_win-x64/app/omni.ja", b"fixture"),
    ]);
    let client = f.zotero.directory.join("client");
    fs::create_dir_all(&client).unwrap();
    f.zotero.extract(&archive, &client).unwrap();
    let app = f.zotero.executable();
    assert_eq!(app, client.join("Zotero_win-x64").join("zotero.exe"));
    assert!(app.parent().unwrap().join("app/omni.ja").is_file());
}

#[test]
fn executable_prefers_direct_client_then_first_folder() {
    let f = Fixture::new();
    let app = f.app();
    assert_eq!(f.zotero.executable(), app);
    let direct = f.zotero.directory.join("client/zotero.exe");
    fs::write(&direct, b"x").unwrap();
    assert_eq!(f.zotero.executable(), direct);
}

#[test]
fn signature_requires_valid_status_and_the_official_publisher() {
    for output in [
        r#"{"status":"HashMismatch","subject":"Corporation for Digital Scholarship"}"#,
        r#"{"status":"Valid","subject":"Unrelated publisher"}"#,
        r#"{"status":"Valid","subject":null}"#,
        "not json",
        "[]",
    ] {
        assert!(check_signature(true, output).is_err(), "accepted {output}");
    }
    let valid = r#"{"status":"Valid","subject":"CN=Corporation for Digital Scholarship"}"#;
    assert!(check_signature(false, valid).is_err());
    assert!(check_signature(true, valid).is_ok());
}

/// A one-shot HTTP server answering with `status` and `headers`.
fn serve_once(status_line: &'static str, headers: &'static str) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buffer = [0u8; 4096];
            let _ = stream.read(&mut buffer);
            let _ = write!(
                stream,
                "HTTP/1.1 {status_line}\r\n{headers}Content-Length: 0\r\nConnection: close\r\n\r\n"
            );
        }
    });
    port
}

#[test]
fn status_requires_zotero_identity() {
    for (headers, expected) in [
        ("", false),
        ("Zotero-API-Version: 3\r\n", false),
        (
            "X-Zotero-Version: 10.0.6\r\nZotero-API-Version: 3\r\nZotero-Server-ID: abc\r\n",
            true,
        ),
    ] {
        let port = serve_once("200 OK", headers);
        let state = status(port);
        assert_eq!(state["running"], json!(expected), "{headers}");
        assert_eq!(state["apiEnabled"], json!(expected));
        let keys: Vec<&str> = state.keys().map(String::as_str).collect();
        assert_eq!(
            keys,
            ["running", "apiEnabled", "apiVersion", "serverID", "port"]
        );
    }
}

#[test]
fn disabled_api_is_distinguished_from_a_foreign_http_error() {
    for (headers, expected) in [
        ("", false),
        (
            "X-Zotero-Version: 10.0.6\r\nZotero-API-Version: 3\r\n",
            true,
        ),
    ] {
        let port = serve_once("403 Forbidden", headers);
        let state = status(port);
        assert_eq!(state["running"], json!(expected));
        assert_eq!(state["apiEnabled"], json!(false));
        assert_eq!(state["status"], json!(403));
    }
}

#[test]
fn redirects_are_not_followed() {
    let port = serve_once("302 Found", "Location: http://example.invalid/\r\n");
    let state = status(port);
    assert_eq!(state["running"], json!(false));
    assert_eq!(state["status"], json!(302));
}

#[test]
fn closed_port_is_not_running() {
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let state = status(port);
    assert_eq!(
        python_json(&Value::Object(state)),
        format!("{{\"running\": false, \"apiEnabled\": false, \"port\": {port}}}")
    );
}

#[test]
fn default_profile_launch_accepts_uri_remoting_and_hidden_window() {
    let f = Fixture::new();
    let mut fake = Fake {
        app: f.app(),
        ..Default::default()
    };
    let uri = "zotero://open-pdf/library/items/ABCD2345?page=1&annotation=BCDE3456";
    let result = f
        .zotero
        .start(23119, None, Some(uri), true, &mut fake)
        .unwrap();
    let (command, hidden) = &fake.spawned[0];
    assert!(hidden);
    assert!(!command.contains(&OsString::from("-no-remote")));
    let profile = f.zotero.directory.join("profile");
    let at = command.iter().position(|part| part == "-profile").unwrap();
    assert_eq!(command[at + 1], profile.clone().into_os_string());
    assert_eq!(
        command[command.len() - 2..],
        [OsString::from("-url"), OsString::from(uri)]
    );
    assert_eq!(result["profile"], json!(profile.display().to_string()));
    let keys: Vec<&str> = result.keys().map(String::as_str).collect();
    assert_eq!(keys, ["started", "pid", "profile", "port"]);
}

#[test]
fn explicit_test_profile_uses_no_remote_and_refuses_an_occupied_port() {
    let f = Fixture::new();
    let profile = f.root.join(".tools").join("test-instance").join("profile");
    let mut fake = Fake {
        app: f.app(),
        ..Default::default()
    };
    f.zotero
        .start(23129, Some(&profile), None, false, &mut fake)
        .unwrap();
    let command = &fake.spawned[0].0;
    assert!(command.contains(&OsString::from("-no-remote")));
    assert!(command.contains(&profile.clone().into_os_string()));
    let mut busy = Fake {
        running: true,
        app: f.app(),
        ..Default::default()
    };
    assert!(f
        .zotero
        .start(23129, Some(&profile), None, false, &mut busy)
        .is_err());
    assert!(busy.spawned.is_empty());
}

#[test]
fn running_default_instance_is_reused_without_touching_profile() {
    let f = Fixture::new();
    let mut fake = Fake {
        running: true,
        server_id: Some("existing"),
        ..Default::default()
    };
    let result = f.zotero.start(23119, None, None, false, &mut fake).unwrap();
    assert_eq!(result["serverID"], json!("existing"));
    assert_eq!(fake.setup_calls, 0);
    assert!(fake.spawned.is_empty());
    assert!(!f.zotero.directory.join("profile").exists());
}

#[test]
fn invalid_uris_and_outside_profiles_are_rejected_before_any_network_or_launch() {
    let f = Fixture::new();
    for uri in [
        "https://example.test",
        "zotero://open-pdf/library/../../file",
        "zotero://select/groups/12/items/invalid",
        "zotero://open-pdf/library/items/ABCD2345?page=0",
        "zotero://open-pdf/library/items/ABCD2345?file=C:/private",
        "zotero://open-pdf/library/items/ABCD2345#fragment",
        "zotero://open-pdf/library/items/ABCD2345?page=1&page=2",
        "zotero://open-pdf/library/items/abcd2345",
        "zotero://open-pdf/library",
        "zotero://select/library\n",
    ] {
        let mut fake = Fake::default();
        assert!(
            f.zotero
                .start(23119, None, Some(uri), false, &mut fake)
                .is_err(),
            "accepted {uri}"
        );
        assert_eq!(fake.status_calls, 0);
        assert!(fake.spawned.is_empty());
    }
    let mut fake = Fake::default();
    assert!(f
        .zotero
        .start(23119, Some(&f.root.join("outside")), None, false, &mut fake)
        .is_err());
    assert_eq!(fake.status_calls, 0);
}

#[test]
fn valid_uris() {
    for uri in [
        "zotero://select/library",
        "zotero://select/groups/12",
        "zotero://select/library/items/ABCD2345",
        "zotero://open-pdf/groups/3/items/ABCD2345?page=12",
        "zotero://open-pdf/library/items/ABCD2345?annotation=BCDE3456",
        "ZOTERO://select/library",
    ] {
        assert!(validate_uri(uri).is_ok(), "rejected {uri}");
    }
}

#[test]
fn ports_outside_the_user_range_are_rejected() {
    for port in [0, 80, 1023, 65536] {
        assert_eq!(
            check_port(port).unwrap_err().to_string(),
            "Port must be between 1024 and 65535"
        );
    }
    assert_eq!(check_port(23119).unwrap(), 23119);
}

#[test]
fn python_json_layout() {
    let value = json!({"b": "中\\\"", "a": [1, true, null]});
    assert_eq!(
        python_json(&value),
        r#"{"b": "\u4e2d\\\"", "a": [1, true, null]}"#
    );
}

#[test]
fn msvc_argument_quoting() {
    assert_eq!(quote_argument("plain"), "plain");
    assert_eq!(
        quote_argument("C:\\Program Files\\Z\\"),
        "\"C:\\Program Files\\Z\\\\\""
    );
    assert_eq!(quote_argument("a\"b"), "\"a\\\"b\"");
    assert_eq!(quote_argument(""), "\"\"");
}
