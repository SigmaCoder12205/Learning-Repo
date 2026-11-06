//! Small OAuth client. All credential validation and user records belong to Keycloak.
use reqwest::{
    blocking::{Client, Response},
    redirect::Policy,
    Url,
};
use serde::Deserialize;
use serde_json::Value;
use std::{io::Read, time::Duration};
use zeroize::{Zeroize, Zeroizing};

pub(super) const CLIENT_ID: &str = "cppbench-cli";
pub(super) const DEFAULT_ISSUER: &str = "http://127.0.0.1:8787/realms/cppbench";
#[derive(Deserialize)]
pub(super) struct Tokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub token_type: String,
    #[serde(default)]
    pub scope: String,
}
impl Drop for Tokens {
    fn drop(&mut self) {
        self.access_token.zeroize();
        self.refresh_token.zeroize();
    }
}
pub(super) enum Error {
    SignedOut,
    Other(String),
}
impl From<Error> for String {
    fn from(e: Error) -> String {
        match e {
            Error::SignedOut => "login is no longer valid; run cppbench account login".into(),
            Error::Other(e) => e,
        }
    }
}
pub(super) struct Provider {
    pub issuer: String,
    client: Client,
}
impl Provider {
    pub fn validate_issuer(issuer: &str) -> Result<Url, String> {
        let url = Url::parse(issuer).map_err(|_| "invalid Keycloak issuer URL")?;
        let local = matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"));
        if (url.scheme() != "https" && !(url.scheme() == "http" && local))
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || url.host_str().is_none()
        {
            return Err("issuer must use HTTPS (HTTP is allowed only on loopback), without credentials, query or fragment".into());
        }
        Ok(url)
    }
    pub fn new(issuer: &str) -> Result<Self, String> {
        let issuer = Self::validate_issuer(issuer)?
            .as_str()
            .trim_end_matches('/')
            .to_owned();
        let client = Client::builder()
            .no_proxy()
            .redirect(Policy::none())
            .connect_timeout(Duration::from_secs(3))
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|_| "cannot initialize account HTTP client")?;
        Ok(Self { issuer, client })
    }
    pub fn endpoint(&self, suffix: &str) -> String {
        format!("{}/protocol/openid-connect/{suffix}", self.issuer)
    }
    fn request_error(_: reqwest::Error) -> Error {
        Error::Other("cannot reach Keycloak; start the local service with ./auth/scripts/setup.sh (saved login retained)".into())
    }
    fn json(response: Response) -> Result<Value, Error> {
        let status = response.status();
        if status.as_u16() == 401 {
            return Err(Error::SignedOut);
        }
        let mut bytes = Zeroizing::new(Vec::new());
        response
            .take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| Error::Other("cannot read Keycloak response".into()))?;
        if bytes.len() > 1024 * 1024 {
            return Err(Error::Other("Keycloak response exceeds 1 MiB".into()));
        }
        let value: Value = serde_json::from_slice(&bytes).map_err(|_| {
            Error::Other(format!(
                "invalid Keycloak JSON response (HTTP {})",
                status.as_u16()
            ))
        })?;
        if !status.is_success() {
            if (status.as_u16() == 400 && value["error"] == "invalid_grant")
                || status.as_u16() == 401
            {
                return Err(Error::SignedOut);
            }
            return Err(Error::Other(format!(
                "Keycloak rejected the request (HTTP {}); check the server/client configuration",
                status.as_u16()
            )));
        }
        Ok(value)
    }
    fn token(&self, values: &[(&str, &str)]) -> Result<Tokens, Error> {
        let response = self
            .client
            .post(self.endpoint("token"))
            .form(values)
            .send()
            .map_err(Self::request_error)?;
        let token: Tokens = serde_json::from_value(Self::json(response)?)
            .map_err(|_| Error::Other("Keycloak returned an incomplete token response".into()))?;
        if !token.token_type.eq_ignore_ascii_case("bearer") || token.access_token.is_empty() {
            return Err(Error::Other(
                "Keycloak returned an unsupported token".into(),
            ));
        }
        Ok(token)
    }
    pub fn exchange(
        &self,
        code: &str,
        verifier: &str,
        redirect_uri: &str,
    ) -> Result<Tokens, Error> {
        self.token(&[
            ("grant_type", "authorization_code"),
            ("client_id", CLIENT_ID),
            ("code", code),
            ("code_verifier", verifier),
            ("redirect_uri", redirect_uri),
        ])
    }
    pub fn refresh(&self, token: &str) -> Result<Tokens, Error> {
        self.token(&[
            ("grant_type", "refresh_token"),
            ("client_id", CLIENT_ID),
            ("refresh_token", token),
        ])
    }
    pub fn identity(&self, access_token: &str) -> Result<Value, Error> {
        let response = self
            .client
            .get(self.endpoint("userinfo"))
            .bearer_auth(access_token)
            .send()
            .map_err(Self::request_error)?;
        let user = Self::json(response)?;
        let id = user["sub"]
            .as_str()
            .filter(|v| !v.is_empty())
            .ok_or_else(|| Error::Other("Keycloak userinfo is missing the account ID".into()))?;
        let username = user["preferred_username"]
            .as_str()
            .filter(|v| !v.is_empty())
            .ok_or_else(|| Error::Other("Keycloak userinfo is missing the username".into()))?;
        Ok(
            serde_json::json!({"id": id, "username": username, "email": user.get("email").and_then(Value::as_str)}),
        )
    }
    pub fn revoke(&self, token: &str) -> Result<(), Error> {
        let response = self
            .client
            .post(self.endpoint("revoke"))
            .form(&[
                ("client_id", CLIENT_ID),
                ("token", token),
                ("token_type_hint", "refresh_token"),
            ])
            .send()
            .map_err(Self::request_error)?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(Error::Other(format!(
                "Keycloak could not revoke the login (HTTP {}); saved login retained",
                response.status().as_u16()
            )))
        }
    }
}
