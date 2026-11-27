use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub hourly_rate: f64,
    pub currency: String,
    pub color: String,
    pub is_active: bool,
    pub created_at: String,
    pub total_seconds: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Client {
    pub id: String,
    pub profile_id: String,
    pub name: String,
    pub contact: String,
    pub email: String,
    pub phone: String,
    pub address: String,
    pub notes: String,
    pub hourly_rate: Option<f64>,
    pub archived: bool,
    pub created_at: String,
    pub total_seconds: i64,
    pub unpaid_amount: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub profile_id: String,
    pub client_id: Option<String>,
    pub client_name: Option<String>,
    pub name: String,
    pub description: String,
    pub status: String,
    pub hourly_rate: Option<f64>,
    pub budget: Option<f64>,
    pub color: String,
    pub archived: bool,
    pub created_at: String,
    pub total_seconds: i64,
    pub unpaid_amount: f64,
    pub last_used_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub id: String,
    pub profile_id: String,
    pub project_id: Option<String>,
    pub project_name: Option<String>,
    pub client_id: Option<String>,
    pub client_name: Option<String>,
    pub description: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub duration_sec: i64,
    pub rate: f64,
    pub amount: f64,
    pub billable: bool,
    pub paid: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Payment {
    pub id: String,
    pub profile_id: String,
    pub client_id: Option<String>,
    pub client_name: Option<String>,
    pub project_id: Option<String>,
    pub project_name: Option<String>,
    pub amount: f64,
    pub currency: String,
    pub method: String,
    pub note: String,
    pub paid_at: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NotificationItem {
    pub id: String,
    pub title: String,
    pub body: String,
    pub kind: String,
    pub created_at: String,
    pub read: bool,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TimerSnapshot {
    pub active: Option<Entry>,
    pub elapsed_sec: i64,
    pub running: bool,
    pub project_name: Option<String>,
    pub client_name: Option<String>,
    pub description: String,
    pub rate: f64,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SummaryRow {
    pub key: String,
    pub label: String,
    pub seconds: i64,
    pub earnings: f64,
    pub color: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub from: String,
    pub to: String,
    pub total_seconds: i64,
    pub billable_seconds: i64,
    pub total_earnings: f64,
    pub paid_earnings: f64,
    pub unpaid_earnings: f64,
    pub entry_count: i64,
    pub by_day: Vec<SummaryRow>,
    pub by_client: Vec<SummaryRow>,
    pub by_project: Vec<SummaryRow>,
    pub entries: Vec<Entry>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DayStat {
    pub date: String,
    pub label: String,
    pub seconds: i64,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Dashboard {
    pub today_seconds: i64,
    pub week_seconds: i64,
    pub month_seconds: i64,
    pub today_earnings: f64,
    pub week_earnings: f64,
    pub unpaid_total: f64,
    pub paid_total: f64,
    pub entries_today: i64,
    pub active_projects: i64,
    pub active_clients: i64,
    pub week_days: Vec<DayStat>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Bootstrap {
    pub version: String,
    pub settings: std::collections::HashMap<String, String>,
    pub profiles: Vec<Profile>,
    pub active_profile_id: String,
    pub timer: TimerSnapshot,
    pub floating_visible: bool,
    pub autostart_enabled: bool,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAsset {
    pub name: String,
    pub url: String,
    pub size: u64,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub has_update: bool,
    pub title: String,
    pub notes: String,
    pub page_url: String,
    pub published_at: String,
    pub assets: Vec<UpdateAsset>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DriveFile {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub size: u64,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DriveStatus {
    pub configured: bool,
    pub connected: bool,
    pub account: String,
    pub folder_id: String,
    pub last_backup_at: String,
    pub files: Vec<DriveFile>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientInput {
    pub id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub contact: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub phone: String,
    #[serde(default)]
    pub address: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub hourly_rate: Option<f64>,
    #[serde(default)]
    pub archived: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInput {
    pub id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub client_id: Option<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub hourly_rate: Option<f64>,
    #[serde(default)]
    pub budget: Option<f64>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub archived: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryInput {
    pub id: Option<String>,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub description: String,
    pub started_at: String,
    pub ended_at: String,
    #[serde(default = "default_true")]
    pub billable: bool,
    #[serde(default)]
    pub paid: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentInput {
    pub id: Option<String>,
    #[serde(default)]
    pub client_id: Option<String>,
    #[serde(default)]
    pub project_id: Option<String>,
    pub amount: f64,
    #[serde(default)]
    pub method: String,
    #[serde(default)]
    pub note: String,
    pub paid_at: String,
    #[serde(default)]
    pub mark_entries_paid: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileInput {
    pub id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub hourly_rate: f64,
    #[serde(default)]
    pub currency: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
}

fn default_true() -> bool {
    true
}
