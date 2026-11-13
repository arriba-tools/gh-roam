//! Runs a command against the project's gist.

use anyhow::Result;

use crate::api::Client;
use crate::api::gists::GistClient;
use crate::cli::{Cli, Command};
use crate::project::ProjectConfig;
use crate::prompt;
use crate::sync;

pub fn run(client: &Client, args: Cli) -> Result<()> {
    let config = ProjectConfig::new(args.name, args.root, args.private)?;
    let client = GistClient::new(client);

    let gist_id = client.find_gist(&config.gist_description())?;

    match args.command {
        Some(Command::Add { files, platform }) => {
            sync::do_add(&client, &config, &files, platform, gist_id.as_deref())?;
        }
        Some(Command::Remove { files, platform }) => {
            sync::do_remove(&client, &config, &files, platform, gist_id.as_deref())?;
        }
        Some(Command::Cleanup) => {
            sync::do_cleanup(&client, &config, gist_id.as_deref())?;
        }
        Some(Command::Delete) => {
            let prompt = format!("Delete the {} gist?", config.gist_description());
            if prompt::confirm(&prompt, args.yes)? {
                sync::do_delete(&client, gist_id.as_deref())?;
            }
        }
        Some(Command::Open) => {
            sync::do_open(gist_id.as_deref())?;
        }
        Some(Command::Sync { files }) => {
            sync::do_sync(&client, &config, &files, gist_id.as_deref())?;
        }
        Some(Command::Pull { files }) => {
            sync::do_pull(&client, &config, &files, gist_id.as_deref())?;
        }
        Some(Command::Status { files }) => {
            sync::do_status(&client, &config, &files, gist_id.as_deref())?;
        }
        Some(Command::Restore { files }) => {
            sync::do_restore(&client, &config, &files, gist_id.as_deref())?;
        }
        None => {
            sync::do_sync(&client, &config, &[], gist_id.as_deref())?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::Client;
    use mockito::{Matcher, Server, ServerGuard};
    use serde_json::json;
    use std::path::PathBuf;

    /// A scratch project root, removed on drop.
    struct Root(PathBuf);

    impl Root {
        fn new(tag: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("roam-{}-{tag}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }

    impl Drop for Root {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn client(server: &ServerGuard) -> Client {
        Client::for_tests(&server.url())
    }

    fn args(root: &Root, command: Option<Command>) -> Cli {
        Cli {
            command,
            name: Some("proj".into()),
            root: Some(root.0.clone()),
            private: true,
            yes: true,
        }
    }

    /// List the user's gists, with `proj config-sync` as g1 when `exists`.
    fn list_gists(server: &mut ServerGuard, exists: bool) {
        let mut gists = vec![json!({ "id": "other", "description": "other config-sync" })];
        if exists {
            gists.push(json!({ "id": "g1", "description": "proj config-sync" }));
        }
        server
            .mock("GET", "/gists")
            .match_query(Matcher::Any)
            .with_body(json!(gists).to_string())
            .create();
    }

    #[test]
    fn no_command_syncs_into_the_given_root() {
        let root = Root::new("default");
        let mut server = Server::new();
        list_gists(&mut server, true);
        let manifest = "name: proj\ndescription: d\nurl: u\nfiles:\n- path: a.txt\n  gist: proj_a.txt\n  updated_at: 2024-01-01T00:00:00Z\n";
        server
            .mock("GET", "/gists/g1")
            .with_body(
                json!({
                    "id": "g1",
                    "files": {
                        ".proj-config-sync.yaml": { "content": manifest },
                        "proj_a.txt": { "content": "from gist" }
                    }
                })
                .to_string(),
            )
            .create();

        run(&client(&server), args(&root, None)).unwrap();
        let pulled = std::fs::read_to_string(root.0.join("a.txt")).unwrap();
        assert_eq!(pulled, "from gist\n");
    }

    #[test]
    fn add_without_a_gist_creates_a_private_one() {
        let root = Root::new("add");
        std::fs::write(root.0.join("a.txt"), "a").unwrap();
        let mut server = Server::new();
        list_gists(&mut server, false);
        let create = server
            .mock("POST", "/gists")
            .match_body(Matcher::PartialJson(json!({
                "description": "proj config-sync",
                "public": false,
                "files": { "proj_a.txt": { "content": "a" } }
            })))
            .with_body(r#"{"id": "g2"}"#)
            .expect(1)
            .create();

        let command = Command::Add {
            files: vec!["a.txt".into()],
            platform: false,
        };
        run(&client(&server), args(&root, Some(command))).unwrap();
        create.assert();
    }

    #[test]
    fn delete_with_yes_deletes_the_project_gist() {
        let root = Root::new("delete");
        let mut server = Server::new();
        list_gists(&mut server, true);
        let delete = server
            .mock("DELETE", "/gists/g1")
            .with_status(204)
            .expect(1)
            .create();
        run(&client(&server), args(&root, Some(Command::Delete))).unwrap();
        delete.assert();
    }

    #[test]
    fn commands_needing_a_gist_fail_without_one() {
        let root = Root::new("missing");
        let mut server = Server::new();
        list_gists(&mut server, false);
        let client = client(&server);

        // Status reports the missing gist instead of failing.
        let status = Command::Status { files: vec![] };
        run(&client, args(&root, Some(status))).unwrap();

        for command in [
            Command::Cleanup,
            Command::Open,
            Command::Pull { files: vec![] },
            Command::Restore { files: vec![] },
            Command::Remove {
                files: vec!["a".into()],
                platform: false,
            },
            Command::Sync { files: vec![] },
        ] {
            assert!(run(&client, args(&root, Some(command))).is_err());
        }
    }
}
