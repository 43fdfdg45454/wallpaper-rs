pub(crate) const NOTICES: &str = "THIRD-PARTY-NOTICES.txt";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    Model,
    Runtime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Asset {
    pub id: &'static str,
    pub title: &'static str,
    pub kind: Part,
    pub urls: &'static [&'static str],
    pub bytes: u64,
    pub sha256: &'static str,
    pub license: &'static str,
    pub license_url: &'static str,
    pub license_text: &'static str,
    pub file: &'static str,
    pub member: &'static str,
    pub notices: &'static str,
    pub file_sha256: &'static str,
}

pub const MODEL: Asset = Asset {
    id: "depth-anything-v2-small",
    title: "Depth Anything V2 Small",
    kind: Part::Model,
    urls: &[
        "https://github.com/fabio-sim/Depth-Anything-ONNX/releases/download/v2.0.0/depth_anything_v2_vits_dynamic.onnx",
    ],
    bytes: 99_092_268,
    sha256: "46c4e8eeda3a27f34701831b6a2ec7753d7b38779b215acb5633424703deed8f",
    license: "Apache-2.0",
    license_url: "https://github.com/DepthAnything/Depth-Anything-V2/blob/main/LICENSE",
    license_text: include_str!("../licenses/depth-anything-v2.txt"),
    file: "depth_anything_v2_vits_dynamic.onnx",
    member: "",
    notices: "",
    file_sha256: "46c4e8eeda3a27f34701831b6a2ec7753d7b38779b215acb5633424703deed8f",
};

#[cfg(all(target_os = "linux", target_env = "gnu", any(target_arch = "x86_64", target_arch = "aarch64")))]
const ORT: Asset = Asset {
    id: "onnxruntime",
    title: "ONNX Runtime 1.31.0",
    kind: Part::Runtime,
    urls: &[],
    bytes: 0,
    sha256: "",
    license: "MIT",
    license_url: "https://github.com/microsoft/onnxruntime/blob/v1.31.0/LICENSE",
    license_text: include_str!("../licenses/onnxruntime.txt"),
    file: "libonnxruntime.so.1.31.0",
    member: "",
    notices: "",
    file_sha256: "",
};

#[cfg(all(target_os = "linux", target_env = "gnu", target_arch = "x86_64"))]
pub const RUNTIME: Option<Asset> = Some(Asset {
    urls: &[
        "https://github.com/microsoft/onnxruntime/releases/download/v1.31.0/onnxruntime-linux-x64-1.31.0.tgz",
    ],
    bytes: 11_408_003,
    sha256: "cc5c72baf5ae5c8238a6841f0897227be2d02826b9cf98eaf02fdefaeeb03a57",
    member: "onnxruntime-linux-x64-1.31.0/lib/libonnxruntime.so.1.31.0",
    notices: "onnxruntime-linux-x64-1.31.0/ThirdPartyNotices.txt",
    file_sha256: "469849977cb60744ea160927e1a3a0643fd4c55c0aff743251c6d293402e334e",
    ..ORT
});

#[cfg(all(target_os = "linux", target_env = "gnu", target_arch = "aarch64"))]
pub const RUNTIME: Option<Asset> = Some(Asset {
    urls: &[
        "https://github.com/microsoft/onnxruntime/releases/download/v1.31.0/onnxruntime-linux-aarch64-1.31.0.tgz",
    ],
    bytes: 10_331_104,
    sha256: "c5b8b3cca31f3d643a3b313b8f42e2d0f16a4fcefd02cda343d97bd5306afaab",
    member: "onnxruntime-linux-aarch64-1.31.0/lib/libonnxruntime.so.1.31.0",
    notices: "onnxruntime-linux-aarch64-1.31.0/ThirdPartyNotices.txt",
    file_sha256: "595c7f7a6775f5c7b3d567c3ed94c04f722092575abcc092560f5208a54daf99",
    ..ORT
});

#[cfg(not(all(
    target_os = "linux",
    target_env = "gnu",
    any(target_arch = "x86_64", target_arch = "aarch64")
)))]
pub const RUNTIME: Option<Asset> = None;

impl Asset {
    pub fn hosts(&self) -> String {
        let mut seen = std::collections::BTreeSet::new();
        let hosts = self.urls.iter().filter_map(|u| u.split('/').nth(2)).filter(|h| seen.insert(*h));
        hosts.collect::<Vec<_>>().join(", ")
    }

    pub fn archive(&self) -> &'static str {
        self.urls.first().and_then(|u| u.rsplit('/').next()).unwrap_or(self.file)
    }

    pub fn notice(&self) -> String {
        let (title, license, url, sha) = (self.title, self.license, self.license_url, self.sha256);
        let sources = self.urls.join("\n        ");
        let third =
            if self.notices.is_empty() { String::new() } else { format!("Third-party notices: {NOTICES}\n") };
        format!(
            "{title}\nLicense: {license} ({url}), full text in LICENSE.txt\n{third}Source: {sources}\nSHA-256: {sha}\n"
        )
    }
}

pub fn assets() -> Vec<Asset> {
    std::iter::once(MODEL).chain(RUNTIME).collect()
}

pub fn find(id: &str) -> Option<Asset> {
    assets().into_iter().find(|a| a.id == id)
}
