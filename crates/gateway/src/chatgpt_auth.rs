use anyhow::{Context, Result, bail, ensure};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header, jwk::JwkSet};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;

const ISSUER: &str = "https://auth.openai.com";
const AUTHORIZE: &str = "https://auth.openai.com/api/accounts/authorize";
const TOKEN: &str = "https://auth.openai.com/api/accounts/oauth/token";
const RESOURCE: &str = "https://api.openai.com/v1";
const PLAN_SCOPE: &str = "chatgpt.tokens.use.direct";
const SCOPES: &str =
    "openid profile email offline_access resource.invoke chatgpt.tokens.use.direct";

#[derive(Default, Serialize, Deserialize)]
struct Store {
    host_id: String,
    active: Option<String>,
    accounts: Vec<Account>,
}

#[derive(Clone, Serialize, Deserialize)]
struct Account {
    id: String,
    client_id: String,
    subject: Option<String>,
    email: Option<String>,
    credentials: Option<Credentials>,
}

#[derive(Clone, Serialize, Deserialize)]
struct Credentials {
    access_token: String,
    refresh_token: String,
    id_token: String,
    expires_at: u64,
    scopes: Vec<String>,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    id_token: Option<String>,
    token_type: String,
    expires_in: u64,
    scope: Option<String>,
}

#[derive(Clone, Deserialize)]
struct Identity {
    sub: String,
    nonce: Option<String>,
    email: Option<String>,
}

#[derive(Deserialize)]
struct Discovery {
    issuer: String,
    jwks_uri: String,
    revocation_endpoint: String,
}

#[derive(Clone)]
pub struct ChatGptAuth {
    directory: PathBuf,
    http: reqwest::Client,
    #[cfg(test)]
    token_endpoint: Option<String>,
}

pub struct AccountInfo {
    pub id: String,
    pub email: Option<String>,
    pub active: bool,
    pub connected: bool,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn random_value() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

fn challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

fn private_options() -> OpenOptions {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
}

fn check_private(path: &Path) -> Result<()> {
    let metadata = std::fs::symlink_metadata(path)?;
    ensure!(
        !metadata.file_type().is_symlink(),
        "ChatGPT storage must not use symbolic links"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        ensure!(
            metadata.permissions().mode() & 0o077 == 0,
            "ChatGPT storage permissions must be owner-only"
        );
    }
    Ok(())
}

impl ChatGptAuth {
    pub fn new() -> Result<Self> {
        let directory = directories::ProjectDirs::from("", "", "codelight")
            .context("Cannot locate the Codelight configuration directory")?
            .config_dir()
            .join("chatgpt");
        Self::at(directory)
    }

    fn at(directory: PathBuf) -> Result<Self> {
        #[cfg(not(unix))]
        bail!("ChatGPT credential storage currently requires a Unix platform");
        if !directory.exists() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                std::fs::DirBuilder::new()
                    .recursive(true)
                    .mode(0o700)
                    .create(&directory)?;
            }
        }
        check_private(&directory)?;
        Ok(Self {
            directory,
            #[cfg(test)]
            token_endpoint: None,
            http: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(Duration::from_secs(10))
                .timeout(Duration::from_secs(30))
                .build()?,
        })
    }

    async fn lock(&self) -> Result<File> {
        let path = self.directory.join("auth.lock");
        let file = match private_options().open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                check_private(&path)?;
                OpenOptions::new().write(true).open(&path)?
            }
            Err(error) => return Err(error.into()),
        };
        tokio::time::timeout(Duration::from_secs(45), async {
            loop {
                match fs2::FileExt::try_lock_exclusive(&file) {
                    Ok(()) => return Ok(file),
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        tokio::time::sleep(Duration::from_millis(50)).await;
                    }
                    Err(error) => return Err(error.into()),
                }
            }
        })
        .await
        .context("Another Codelight process is updating ChatGPT credentials")?
    }

    fn read(&self) -> Result<Store> {
        let path = self.directory.join("accounts.json");
        match std::fs::symlink_metadata(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Store::default()),
            Err(error) => Err(error.into()),
            Ok(_) => {
                check_private(&path)?;
                serde_json::from_slice(&std::fs::read(path)?)
                    .map_err(|_| anyhow::anyhow!("Invalid ChatGPT credential store"))
            }
        }
    }

    fn save(&self, store: &Store) -> Result<()> {
        let path = self.directory.join("accounts.json");
        let temporary = self.directory.join(format!("{}.tmp", Uuid::new_v4()));
        let result = (|| -> Result<()> {
            let mut file = private_options().open(&temporary)?;
            file.write_all(&serde_json::to_vec(store)?)?;
            file.sync_all()?;
            std::fs::rename(&temporary, path)?;
            File::open(&self.directory)?.sync_all()?;
            Ok(())
        })();
        if result.is_err() {
            std::fs::remove_file(temporary).ok();
        }
        result
    }

    pub async fn accounts(&self) -> Result<Vec<AccountInfo>> {
        let _lock = self.lock().await?;
        let store = self.read()?;
        Ok(store
            .accounts
            .iter()
            .map(|account| AccountInfo {
                id: account.id.clone(),
                email: account.email.clone(),
                active: store.active.as_ref() == Some(&account.id),
                connected: account.credentials.is_some(),
            })
            .collect())
    }

    pub async fn select(&self, id: &str) -> Result<()> {
        let _lock = self.lock().await?;
        let mut store = self.read()?;
        let account = store
            .accounts
            .iter()
            .find(|account| account.id == id)
            .context("Unknown ChatGPT account ID")?;
        ensure!(
            account.credentials.is_some(),
            "This account needs login first: codelight login --account {id}"
        );
        store.active = Some(id.to_string());
        self.save(&store)
    }

    pub async fn active_id(&self) -> Result<String> {
        let _lock = self.lock().await?;
        self.read()?
            .active
            .context("Sign in first with codelight login")
    }

    async fn discovery(&self) -> Result<Discovery> {
        let response = self
            .http
            .get(format!("{ISSUER}/.well-known/openid-configuration"))
            .send()
            .await?;
        ensure!(
            response.status().is_success(),
            "Unable to load OpenAI authentication configuration"
        );
        let discovery: Discovery = response.json().await?;
        ensure!(
            discovery.issuer == ISSUER,
            "Unexpected authentication issuer"
        );
        for endpoint in [&discovery.jwks_uri, &discovery.revocation_endpoint] {
            let url = reqwest::Url::parse(endpoint)?;
            ensure!(
                url.scheme() == "https"
                    && url.host_str() == Some("auth.openai.com")
                    && url.port_or_known_default() == Some(443)
                    && url.username().is_empty()
                    && url.password().is_none(),
                "Unexpected authentication endpoint"
            );
        }
        Ok(discovery)
    }

    async fn identity(&self, token: &str, client: &str, nonce: Option<&str>) -> Result<Identity> {
        let discovery = self.discovery().await?;
        let response = self.http.get(discovery.jwks_uri).send().await?;
        ensure!(
            response.status().is_success(),
            "Unable to load identity verification keys"
        );
        let keys: JwkSet = response.json().await?;
        validate_identity(token, client, nonce, &keys)
    }

    async fn exchange(&self, form: &[(&str, &str)]) -> Result<TokenResponse> {
        let endpoint = TOKEN;
        #[cfg(test)]
        let endpoint = self.token_endpoint.as_deref().unwrap_or(endpoint);
        let response = self.http.post(endpoint).form(form).send().await?;
        ensure!(
            response.status().is_success(),
            "ChatGPT token request failed (HTTP {}). Run codelight login to reconnect.",
            response.status().as_u16()
        );
        response
            .json()
            .await
            .map_err(|_| anyhow::anyhow!("Invalid ChatGPT token response"))
    }

    pub async fn login(&self, account_id: Option<&str>, new_account: bool) -> Result<String> {
        let (host, existing) = {
            let _lock = self.lock().await?;
            let mut store = self.read()?;
            if store.host_id.is_empty() {
                store.host_id = format!("urn:uuid:{}", Uuid::new_v4());
                self.save(&store)?;
            }
            let selected = if new_account {
                None
            } else {
                account_id
                    .or(store.active.as_deref())
                    .or_else(|| store.accounts.last().map(|account| account.id.as_str()))
            };
            let existing = selected
                .map(|id| {
                    store
                        .accounts
                        .iter()
                        .find(|account| account.id == id)
                        .cloned()
                        .context("Unknown ChatGPT account ID")
                })
                .transpose()?;
            (store.host_id, existing)
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let redirect = format!(
            "http://127.0.0.1:{}/auth/callback",
            listener.local_addr()?.port()
        );
        let state = random_value();
        let nonce = random_value();
        let verifier = random_value();
        let client = existing
            .as_ref()
            .map(|account| account.client_id.as_str())
            .unwrap_or("dynamic_agent_client");
        let mut url = reqwest::Url::parse(AUTHORIZE)?;
        url.query_pairs_mut().extend_pairs([
            ("client_id", client),
            ("ext_agent_host_id", host.as_str()),
            ("response_type", "code"),
            ("redirect_uri", redirect.as_str()),
            ("scope", SCOPES),
            ("resource", RESOURCE),
            ("state", state.as_str()),
            ("nonce", nonce.as_str()),
            ("code_challenge_method", "S256"),
            ("code_challenge", challenge(&verifier).as_str()),
        ]);
        if existing.is_none() {
            url.query_pairs_mut()
                .append_pair("agent_name_hint", "Codelight");
        }
        println!("Continue with ChatGPT in your browser. Waiting for authorization...");
        open_browser(url.as_str())?;
        let (code, issued_client) = tokio::time::timeout(
            Duration::from_secs(300),
            callback(
                &listener,
                &state,
                existing.as_ref().map(|account| account.client_id.as_str()),
            ),
        )
        .await
        .context("ChatGPT login timed out; run codelight login again")??;
        let mut account = existing.unwrap_or_else(|| Account {
            id: Uuid::new_v4().to_string(),
            client_id: issued_client.clone(),
            subject: None,
            email: None,
            credentials: None,
        });
        {
            let _lock = self.lock().await?;
            let mut store = self.read()?;
            if !store.accounts.iter().any(|saved| saved.id == account.id) {
                store.accounts.push(account.clone());
            }
            self.save(&store)?;
        }
        let tokens = self
            .exchange(&[
                ("grant_type", "authorization_code"),
                ("client_id", &issued_client),
                ("code", &code),
                ("code_verifier", &verifier),
                ("redirect_uri", &redirect),
                ("resource", RESOURCE),
            ])
            .await?;
        let identity = self
            .identity(
                tokens
                    .id_token
                    .as_deref()
                    .context("Login response has no ID token")?,
                &issued_client,
                Some(&nonce),
            )
            .await?;
        if let Some(subject) = &account.subject {
            ensure!(
                subject == &identity.sub,
                "Login returned a different account; saved credentials were not replaced"
            );
        }
        account.subject = Some(identity.sub);
        account.email = identity.email;
        account.credentials = Some(credentials(tokens, None)?);
        let _lock = self.lock().await?;
        let mut store = self.read()?;
        let saved = store
            .accounts
            .iter_mut()
            .find(|saved| saved.id == account.id)
            .context("Account registration disappeared during login")?;
        *saved = account.clone();
        store.active = Some(account.id.clone());
        self.save(&store)?;
        Ok(account.id)
    }

    pub async fn access_token(&self, id: &str) -> Result<String> {
        let _lock = self.lock().await?;
        let mut store = self.read()?;
        let account = store
            .accounts
            .iter_mut()
            .find(|account| account.id == id)
            .context("ChatGPT account is no longer available")?;
        let saved = account
            .credentials
            .as_ref()
            .context("ChatGPT account is signed out; run codelight login")?;
        ensure!(
            saved.scopes.iter().any(|scope| scope == PLAN_SCOPE),
            "ChatGPT plan usage is not enabled; run codelight login"
        );
        if saved.expires_at > now().saturating_add(60) {
            return Ok(saved.access_token.clone());
        }
        let tokens = self
            .exchange(&[
                ("grant_type", "refresh_token"),
                ("client_id", &account.client_id),
                ("refresh_token", &saved.refresh_token),
                ("resource", RESOURCE),
            ])
            .await?;
        if let Some(id_token) = &tokens.id_token {
            let identity = self.identity(id_token, &account.client_id, None).await?;
            ensure!(
                Some(&identity.sub) == account.subject.as_ref(),
                "Refreshed identity does not match the selected account"
            );
        }
        let updated = credentials(tokens, Some(saved))?;
        let access_token = updated.access_token.clone();
        account.credentials = Some(updated);
        self.save(&store)?;
        Ok(access_token)
    }

    pub async fn logout(&self, account_id: Option<&str>) -> Result<bool> {
        let _lock = self.lock().await?;
        let mut store = self.read()?;
        let id = account_id
            .or(store.active.as_deref())
            .context("No active ChatGPT account")?
            .to_string();
        let account = store
            .accounts
            .iter_mut()
            .find(|account| account.id == id)
            .context("Unknown ChatGPT account ID")?;
        let mut revoked = account.credentials.is_none();
        if let Some(credentials) = &account.credentials
            && let Ok(discovery) = self.discovery().await
        {
            for attempt in 0..3 {
                let response = self
                    .http
                    .post(&discovery.revocation_endpoint)
                    .form(&[
                        ("token", credentials.refresh_token.as_str()),
                        ("token_type_hint", "refresh_token"),
                        ("client_id", account.client_id.as_str()),
                    ])
                    .send()
                    .await;
                match response {
                    Ok(response) if response.status().as_u16() == 200 => {
                        revoked = true;
                        break;
                    }
                    Ok(response) if !response.status().is_server_error() => break,
                    _ => tokio::time::sleep(Duration::from_millis(250 * (1 << attempt))).await,
                }
            }
        }
        account.credentials = None;
        if store.active.as_deref() == Some(&id) {
            store.active = None;
        }
        self.save(&store)?;
        Ok(revoked)
    }
}

fn credentials(tokens: TokenResponse, previous: Option<&Credentials>) -> Result<Credentials> {
    ensure!(
        tokens.token_type.eq_ignore_ascii_case("Bearer")
            && !tokens.access_token.is_empty()
            && tokens.expires_in > 0,
        "Invalid ChatGPT token type or expiration"
    );
    let scopes: Vec<String> = match tokens.scope {
        Some(scope) => scope.split_whitespace().map(str::to_string).collect(),
        None => previous
            .context("Login response did not grant any scopes")?
            .scopes
            .clone(),
    };
    ensure!(
        scopes.iter().any(|scope| scope == PLAN_SCOPE),
        "ChatGPT plan usage was not granted. Sign in again and enable plan usage."
    );
    let refresh_token = tokens
        .refresh_token
        .filter(|token| !token.is_empty())
        .context("Token response is missing a rotating refresh token")?;
    let id_token = tokens
        .id_token
        .or_else(|| previous.map(|saved| saved.id_token.clone()))
        .context("Login response has no ID token")?;
    Ok(Credentials {
        access_token: tokens.access_token,
        refresh_token,
        id_token,
        expires_at: now().saturating_add(tokens.expires_in),
        scopes,
    })
}

fn callback_values(
    target: &str,
    expected_state: &str,
    existing_client: Option<&str>,
) -> Result<(String, String)> {
    ensure!(
        target.starts_with("/auth/callback?"),
        "Unexpected OAuth callback path"
    );
    let url = reqwest::Url::parse(&format!("http://127.0.0.1{target}"))?;
    let mut values = HashMap::new();
    for (key, value) in url.query_pairs() {
        ensure!(
            values
                .insert(key.into_owned(), value.into_owned())
                .is_none(),
            "Duplicate OAuth callback parameter"
        );
    }
    ensure!(
        values.get("state").map(String::as_str) == Some(expected_state),
        "OAuth callback state mismatch"
    );
    ensure!(
        !values.contains_key("error"),
        "ChatGPT authorization was declined or failed"
    );
    let code = values
        .remove("code")
        .filter(|code| !code.is_empty())
        .context("OAuth callback has no code")?;
    let client = match (existing_client, values.remove("client_id")) {
        (Some(expected), Some(actual)) => {
            ensure!(
                expected == actual,
                "OAuth callback changed the registered client ID"
            );
            actual
        }
        (Some(expected), None) => expected.to_string(),
        (None, Some(actual)) if !actual.is_empty() && actual != "dynamic_agent_client" => actual,
        _ => bail!("OAuth registration returned no issued client ID"),
    };
    Ok((code, client))
}

async fn callback(
    listener: &tokio::net::TcpListener,
    state: &str,
    client: Option<&str>,
) -> Result<(String, String)> {
    loop {
        let (mut socket, _) = listener.accept().await?;
        let mut bytes = Vec::new();
        let read = tokio::time::timeout(Duration::from_secs(5), async {
            let mut buffer = [0; 1024];
            while bytes.len() < 16384 {
                let count = socket.read(&mut buffer).await?;
                if count == 0 {
                    break;
                }
                bytes.extend_from_slice(&buffer[..count]);
                if bytes.windows(4).any(|part| part == b"\r\n\r\n") {
                    break;
                }
            }
            Ok::<_, std::io::Error>(())
        })
        .await;
        if !matches!(read, Ok(Ok(()))) {
            continue;
        }
        let request = String::from_utf8_lossy(&bytes);
        let mut parts = request
            .lines()
            .next()
            .unwrap_or_default()
            .split_whitespace();
        let method = parts.next().unwrap_or_default();
        let target = parts.next().unwrap_or_default();
        if method != "GET" || !target.starts_with("/auth/callback?") {
            socket
                .write_all(
                    b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .await
                .ok();
            continue;
        }
        let result = callback_values(target, state, client);
        let (status, body) = if result.is_ok() {
            (
                "200 OK",
                "Authorization received. Return to Codelight to finish signing in.",
            )
        } else {
            (
                "400 Bad Request",
                "Authorization failed. Return to Codelight and try again.",
            )
        };
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        socket.write_all(response.as_bytes()).await.ok();
        return result;
    }
}

fn open_browser(url: &str) -> Result<()> {
    let program = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    let status = std::process::Command::new(program)
        .arg(url)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .context("Unable to open a browser for ChatGPT login")?;
    ensure!(
        status.success(),
        "Unable to open a browser for ChatGPT login"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token_response() -> TokenResponse {
        TokenResponse {
            access_token: "test-access".into(),
            refresh_token: Some("test-refresh".into()),
            id_token: Some("test-identity".into()),
            token_type: "Bearer".into(),
            expires_in: 3600,
            scope: Some(SCOPES.into()),
        }
    }

    #[test]
    fn pkce_matches_rfc7636() {
        assert_eq!(
            challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
        assert_ne!(random_value(), random_value());
    }

    #[test]
    fn callback_requires_state_and_issued_client() {
        for query in [
            "state=wrong&code=test&client_id=issued",
            "state=expected&code=test",
            "state=expected&code=test&client_id=dynamic_agent_client",
            "state=expected&code=test&client_id=issued&state=expected",
            "state=expected&error=denied&code=test&client_id=issued",
        ] {
            assert!(callback_values(&format!("/auth/callback?{query}"), "expected", None).is_err());
        }
        assert_eq!(
            callback_values(
                "/auth/callback?state=expected&code=test&client_id=issued",
                "expected",
                None
            )
            .unwrap(),
            ("test".into(), "issued".into())
        );
    }

    #[test]
    fn returning_callback_cannot_change_registration() {
        assert!(
            callback_values(
                "/auth/callback?state=expected&code=test&client_id=other",
                "expected",
                Some("issued")
            )
            .is_err()
        );
        assert_eq!(
            callback_values(
                "/auth/callback?state=expected&code=test",
                "expected",
                Some("issued")
            )
            .unwrap()
            .1,
            "issued"
        );
    }

    #[test]
    fn tokens_require_plan_grant_and_rotating_refresh() {
        let mut tokens = token_response();
        tokens.scope = Some("openid email".into());
        assert!(credentials(tokens, None).is_err());
        let mut tokens = token_response();
        tokens.refresh_token = None;
        assert!(credentials(tokens, None).is_err());
        let mut tokens = token_response();
        tokens.scope = None;
        assert!(credentials(tokens, None).is_err());
        let previous = credentials(token_response(), None).unwrap();
        let mut tokens = token_response();
        tokens.scope = None;
        tokens.id_token = None;
        tokens.refresh_token = Some("replacement".into());
        let refreshed = credentials(tokens, Some(&previous)).unwrap();
        assert_eq!(refreshed.refresh_token, "replacement");
        assert_eq!(refreshed.scopes, previous.scopes);
        assert_eq!(refreshed.id_token, previous.id_token);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn private_store_and_account_selection_survive_reload() {
        use std::os::unix::fs::PermissionsExt;
        let path = std::env::temp_dir().join(format!("codelight-auth-test-{}", Uuid::new_v4()));
        let auth = ChatGptAuth::at(path.clone()).unwrap();
        let store = Store {
            host_id: "host-test".into(),
            active: Some("one".into()),
            accounts: vec![
                Account {
                    id: "one".into(),
                    client_id: "client-one".into(),
                    subject: Some("subject-one".into()),
                    email: None,
                    credentials: Some(credentials(token_response(), None).unwrap()),
                },
                Account {
                    id: "two".into(),
                    client_id: "client-two".into(),
                    subject: None,
                    email: None,
                    credentials: None,
                },
            ],
        };
        auth.save(&store).unwrap();
        assert_eq!(
            std::fs::metadata(path.join("accounts.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert!(auth.select("two").await.is_err());
        assert!(auth.select("missing").await.is_err());
        assert_eq!(auth.active_id().await.unwrap(), "one");
        assert_eq!(auth.access_token("one").await.unwrap(), "test-access");
        assert!(auth.access_token("two").await.is_err());
        assert!(auth.logout(Some("two")).await.unwrap());
        let reloaded = ChatGptAuth::at(path.clone()).unwrap().read().unwrap();
        assert_eq!(reloaded.host_id, "host-test");
        assert_eq!(reloaded.accounts[1].client_id, "client-two");
        let held = auth.lock().await.unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(100), auth.lock())
                .await
                .is_err()
        );
        drop(held);
        assert!(auth.lock().await.is_ok());
        std::fs::set_permissions(
            path.join("accounts.json"),
            std::fs::Permissions::from_mode(0o644),
        )
        .unwrap();
        assert!(auth.read().is_err());
        std::fs::remove_dir_all(path).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn credential_store_rejects_symlinks() {
        let path = std::env::temp_dir().join(format!("codelight-auth-test-{}", Uuid::new_v4()));
        let auth = ChatGptAuth::at(path.clone()).unwrap();
        std::os::unix::fs::symlink("missing", path.join("accounts.json")).unwrap();
        assert!(auth.read().is_err());
        std::fs::remove_dir_all(path).unwrap();
    }

    #[tokio::test]
    async fn loopback_callback_accepts_browser_redirect() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move { callback(&listener, "expected", None).await });
        let response = reqwest::get(format!(
            "http://{address}/auth/callback?state=expected&code=test&client_id=issued"
        ))
        .await
        .unwrap();
        assert_eq!(response.status(), 200);
        assert_eq!(
            task.await.unwrap().unwrap(),
            ("test".into(), "issued".into())
        );
    }
}

fn validate_identity(
    token: &str,
    client: &str,
    nonce: Option<&str>,
    keys: &JwkSet,
) -> Result<Identity> {
    let header = decode_header(token).map_err(|_| anyhow::anyhow!("Invalid ID token header"))?;
    ensure!(
        header.alg == Algorithm::RS256,
        "Unexpected ID token algorithm"
    );
    let key = keys
        .find(
            header
                .kid
                .as_deref()
                .context("ID token has no signing key ID")?,
        )
        .context("Unknown identity signing key")?;
    let key = DecodingKey::from_jwk(key).context("Invalid identity verification key")?;
    let mut validation = Validation::new(Algorithm::RS256);
    validation.validate_nbf = true;
    validation.set_issuer(&[ISSUER]);
    validation.set_audience(&[client]);
    validation.set_required_spec_claims(&["exp", "iss", "aud", "sub"]);
    let identity = decode::<Identity>(token, &key, &validation)
        .map_err(|_| anyhow::anyhow!("ID token signature or claims are invalid"))?
        .claims;
    ensure!(!identity.sub.is_empty(), "ID token has an empty subject");
    if let Some(nonce) = nonce {
        ensure!(
            identity.nonce.as_deref() == Some(nonce),
            "ID token nonce does not match this login"
        );
    }
    Ok(identity)
}

#[cfg(test)]
mod identity_and_refresh_tests {
    use super::*;
    use jsonwebtoken::{EncodingKey, Header, encode};
    use serde_json::json;

    #[test]
    fn identity_rejects_wrong_signature_issuer_audience_expiry_and_nonce() {
        let rsa = openssl::rsa::Rsa::generate(2048).unwrap();
        let encoding = EncodingKey::from_rsa_pem(&rsa.private_key_to_pem().unwrap()).unwrap();
        let keys: JwkSet = serde_json::from_value(json!({"keys":[{"kty":"RSA", "kid":"test-key", "alg":"RS256", "n":URL_SAFE_NO_PAD.encode(rsa.n().to_vec()), "e":URL_SAFE_NO_PAD.encode(rsa.e().to_vec())}]})).unwrap();
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some("test-key".into());
        let claims = json!({"sub":"test-subject", "iss":ISSUER, "aud":"test-client", "exp":now()+3600, "nonce":"test-nonce"});
        let token = encode(&header, &claims, &encoding).unwrap();
        assert_eq!(
            validate_identity(&token, "test-client", Some("test-nonce"), &keys)
                .unwrap()
                .sub,
            "test-subject"
        );
        for (key, value) in [
            ("iss", json!("https://other.example")),
            ("aud", json!("other-client")),
            ("exp", json!(now() - 3600)),
            ("nonce", json!("other-nonce")),
            ("sub", json!("")),
            ("nbf", json!(now() + 3600)),
        ] {
            let mut changed = claims.clone();
            changed[key] = value;
            let token = encode(&header, &changed, &encoding).unwrap();
            assert!(validate_identity(&token, "test-client", Some("test-nonce"), &keys).is_err());
        }
        let other = openssl::rsa::Rsa::generate(2048).unwrap();
        let token = encode(
            &header,
            &claims,
            &EncodingKey::from_rsa_pem(&other.private_key_to_pem().unwrap()).unwrap(),
        )
        .unwrap();
        assert!(validate_identity(&token, "test-client", Some("test-nonce"), &keys).is_err());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn concurrent_refresh_uses_issued_client_and_saves_replacement_once() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}/token", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut data = Vec::new();
            let mut buffer = [0; 4096];
            loop {
                let count = socket.read(&mut buffer).await.unwrap();
                assert!(count > 0);
                data.extend_from_slice(&buffer[..count]);
                if let Some(boundary) = data.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&data[..boundary]);
                    let size: usize = headers
                        .lines()
                        .find_map(|line| {
                            line.to_lowercase()
                                .strip_prefix("content-length:")
                                .map(|value| value.trim().parse().unwrap())
                        })
                        .unwrap();
                    if data.len() >= boundary + 4 + size {
                        let form = String::from_utf8_lossy(&data[boundary + 4..]);
                        assert!(form.contains("client_id=issued-test"));
                        assert!(form.contains("grant_type=refresh_token"));
                        assert!(!form.contains("scope="));
                        break;
                    }
                }
            }
            let body = json!({"access_token":"new-test-access","refresh_token":"new-test-refresh","token_type":"Bearer","expires_in":3600}).to_string();
            socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
            assert!(
                tokio::time::timeout(Duration::from_millis(250), listener.accept())
                    .await
                    .is_err()
            );
        });
        let path = std::env::temp_dir().join(format!("codelight-refresh-test-{}", Uuid::new_v4()));
        let mut auth = ChatGptAuth::at(path.clone()).unwrap();
        auth.token_endpoint = Some(endpoint);
        auth.save(&Store {
            host_id: "test-host".into(),
            active: Some("test-account".into()),
            accounts: vec![Account {
                id: "test-account".into(),
                client_id: "issued-test".into(),
                subject: Some("test-subject".into()),
                email: None,
                credentials: Some(Credentials {
                    access_token: "old-test-access".into(),
                    refresh_token: "old-test-refresh".into(),
                    id_token: "old-test-id".into(),
                    expires_at: 0,
                    scopes: vec![PLAN_SCOPE.into()],
                }),
            }],
        })
        .unwrap();
        let (first, second) = tokio::join!(
            auth.access_token("test-account"),
            auth.access_token("test-account")
        );
        assert_eq!(first.unwrap(), "new-test-access");
        assert_eq!(second.unwrap(), "new-test-access");
        assert_eq!(
            auth.read().unwrap().accounts[0]
                .credentials
                .as_ref()
                .unwrap()
                .refresh_token,
            "new-test-refresh"
        );
        server.await.unwrap();
        std::fs::remove_dir_all(path).unwrap();
    }
}
