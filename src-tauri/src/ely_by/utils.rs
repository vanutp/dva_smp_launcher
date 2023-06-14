use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElyByUserInfo {
    pub uuid: String,
    pub username: String,
    pub profile_link: String,
}

pub async fn get_user_info(token: &str) -> anyhow::Result<ElyByUserInfo> {
    let client = reqwest::Client::new();
    let user_info = client.get("https://account.ely.by/api/account/v1/info")
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await?
        .error_for_status()?
        .json::<ElyByUserInfo>()
        .await?;
    Ok(user_info)
}
