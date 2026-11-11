use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;
use std::time::Duration;

const VERSION_STUDIO_HASH: &str = "version-012732894899482c";

const BASE_URLS: &[(&str, u64)] = &[
    ("https://setup.rbxcdn.com", 0),
    ("https://setup-aws.rbxcdn.com", 2),
    ("https://setup-ak.rbxcdn.com", 2),
    ("https://roblox-setup.cachefly.net", 2),
    ("https://s3.amazonaws.com/setup.roblox.com", 4),
];

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientVersion {
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub version_guid: String,
    #[serde(default)]
    pub client_version_upload: String,
}

#[derive(Clone, Debug)]
pub struct Package {
    pub name: String,
    pub signature: String,
    pub packed_size: u64,
    pub size: u64,
}

#[derive(Clone, Debug, Default)]
pub struct PackageManifest {
    pub packages: Vec<Package>,
}

impl PackageManifest {
    pub fn parse(data: &str) -> Result<Self> {
        let mut lines = data.lines();
        let version = lines.next().unwrap_or_default().trim();
        if version != "v0" {
            bail!("Unexpected package manifest version: {version:?} (expected v0)");
        }

        let mut packages = Vec::new();

        loop {
            let name = match lines.next() {
                Some(line) if !line.trim().is_empty() => line.trim().to_string(),
                _ => break,
            };
            let signature = match lines.next() {
                Some(line) if !line.trim().is_empty() => line.trim().to_string(),
                _ => break,
            };
            let packed_size: u64 = match lines.next().map(|l| l.trim().parse()) {
                Some(Ok(value)) => value,
                _ => break,
            };
            let size: u64 = match lines.next().map(|l| l.trim().parse()) {
                Some(Ok(value)) => value,
                _ => break,
            };

            if name == "RobloxPlayerLauncher.exe" {
                break;
            }

            packages.push(Package {
                name,
                signature,
                packed_size,
                size,
            });
        }

        Ok(Self { packages })
    }

    pub fn is_empty(&self) -> bool {
        self.packages.is_empty()
    }

    pub fn find(&self, name: &str) -> Option<&Package> {
        self.packages.iter().find(|p| p.name == name)
    }
}

#[derive(Clone)]
pub struct Deployment {
    agent: ureq::Agent,
    pub base_url: Option<String>,
    pub channel: String,
    pub binary_type: String,
}

impl Default for Deployment {
    fn default() -> Self {
        Self::new()
    }
}

impl Deployment {
    pub fn new() -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(6))
            .timeout_read(Duration::from_secs(30))
            .user_agent(&format!(
                "{}/{}",
                crate::paths::PROJECT_NAME,
                crate::paths::PROJECT_VERSION
            ))
            .build();

        Self {
            agent,
            base_url: None,
            channel: "production".into(),
            binary_type: "WindowsPlayer".into(),
        }
    }

    pub fn test_connection(&self, url: &str) -> Result<()> {
        let target = format!("{url}/versionStudio");
        let response = self
            .agent
            .get(&target)
            .call()
            .map_err(|e| anyhow!("{e}"))?;
        let body = response.into_string().context("reading versionStudio")?;
        if body.trim() != VERSION_STUDIO_HASH {
            bail!(
                "versionStudio response does not match (expected {VERSION_STUDIO_HASH:?}, got {body:?})"
            );
        }
        Ok(())
    }

    pub fn initialize_connectivity(&mut self) -> Result<()> {
        crate::logger::log("Deployment", "Testing connectivity...");
        let mut errors = Vec::new();

        let mut urls = BASE_URLS.to_vec();
        urls.sort_by_key(|(_, priority)| *priority);

        for (url, priority) in urls {
            if priority > 0 {
                std::thread::sleep(Duration::from_millis(priority * 250));
            }
            match self.test_connection(url) {
                Ok(()) => {
                    self.base_url = Some(url.to_string());
                    crate::logger::log("Deployment", format!("Using base URL {url}"));
                    return Ok(());
                }
                Err(error) => {
                    crate::logger::warn("Deployment", format!("{url} failed: {error}"));
                    errors.push(format!("{url}: {error}"));
                }
            }
        }

        Err(anyhow!(
            "Unable to connect to Roblox setup servers.\n{}",
            errors.join("\n")
        ))
    }

    pub fn base_url(&self) -> Result<&str> {
        self.base_url
            .as_deref()
            .ok_or_else(|| anyhow!("Connectivity has not been initialized"))
    }

    pub fn location(&self, resource: &str) -> Result<String> {
        Ok(format!("{}/channel/common{resource}", self.base_url()?))
    }

    pub fn get_info(&self) -> Result<ClientVersion> {
        let default_channel = self.channel.eq_ignore_ascii_case("production");
        let path = if default_channel {
            format!("/v2/client-version/{}", self.binary_type)
        } else {
            format!(
                "/v2/client-version/{}/channel/{}",
                self.binary_type, self.channel
            )
        };

        let first = self.get_json::<ClientVersion>(&format!(
            "https://clientsettingscdn.roblox.com{path}"
        ));

        let version = match first {
            Ok(version) => version,
            Err(error) => {
                crate::logger::warn(
                    "Deployment",
                    format!("clientsettingscdn failed ({error}), falling back to clientsettings"),
                );
                self.get_json::<ClientVersion>(&format!("https://clientsettings.roblox.com{path}"))?
            }
        };

        if version.version_guid.is_empty() {
            bail!("Roblox returned an invalid client version");
        }

        Ok(version)
    }

    pub fn get_manifest(&self, version_guid: &str) -> Result<PackageManifest> {
        let url = self.location(&format!("/{version_guid}-rbxPkgManifest.txt"))?;
        let body = self.get_text(&url)?;
        PackageManifest::parse(&body)
    }

    pub fn get_text(&self, url: &str) -> Result<String> {
        let response = self
            .agent
            .get(url)
            .call()
            .map_err(|e| anyhow!("GET {url} failed: {e}"))?;
        response
            .into_string()
            .with_context(|| format!("reading {url}"))
    }

    pub fn get_json<T: for<'de> Deserialize<'de>>(&self, url: &str) -> Result<T> {
        let body = self.get_text(url)?;
        serde_json::from_str(&body).with_context(|| format!("parsing JSON from {url}"))
    }

    pub fn download_reader(
        &self,
        url: &str,
    ) -> Result<Box<dyn std::io::Read + Send + 'static>> {
        let response = self
            .agent
            .get(url)
            .call()
            .map_err(|e| anyhow!("GET {url} failed: {e}"))?;
        Ok(Box::new(response.into_reader()))
    }
}

pub fn parse_version(value: &str) -> Option<(u64, u64, u64, u64)> {
    let value = value.trim_start_matches("version-");
    let mut parts = value.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    let build = parts.next().unwrap_or("0").parse().ok()?;
    Some((major, minor, patch, build))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_package_manifest() {
        let data = "v0\r\ncontent-fonts.zip\r\nabc123\r\n100\r\n200\r\nRobloxApp.zip\r\ndef456\r\n300\r\n400\r\nRobloxPlayerLauncher.exe\r\nfff\r\n1\r\n1\r\n";
        let manifest = PackageManifest::parse(data).unwrap();
        assert_eq!(manifest.packages.len(), 2);
        assert_eq!(manifest.find("content-fonts.zip").unwrap().signature, "abc123");
        assert_eq!(manifest.find("RobloxApp.zip").unwrap().size, 400);
        assert!(manifest.find("RobloxPlayerLauncher.exe").is_none());
    }

    #[test]
    fn parses_version_strings() {
        assert_eq!(
            parse_version("version-1.2.3.4"),
            Some((1, 2, 3, 4))
        );
        assert_eq!(parse_version("version-1.2.3"), Some((1, 2, 3, 0)));
    }
}
