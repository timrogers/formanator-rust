//! HTTP client for the Forma API (`https://api.joinforma.com`).

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use reqwest::blocking::{Client, Response, multipart};
use serde::Deserialize;
use serde_json::Value;

const API_BASE: &str = "https://api.joinforma.com";
const AUTH_HEADER: &str = "x-auth-token";

fn client() -> Result<Client> {
    Client::builder()
        .timeout(Duration::from_secs(120))
        .user_agent(concat!("formanator/", env!("CARGO_PKG_VERSION")))
        .build()
        .context("Failed to build HTTP client")
}

// ---------------------------------------------------------------------------
// Public domain types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize)]
pub struct Benefit {
    pub id: String,
    pub name: String,
    #[serde(rename = "remainingAmount")]
    pub remaining_amount: f64,
    #[serde(rename = "remainingAmountCurrency")]
    pub remaining_amount_currency: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Category {
    pub category_id: String,
    pub category_name: String,
    pub subcategory_name: String,
    pub subcategory_value: String,
    pub subcategory_alias: Option<String>,
    pub benefit_id: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct BenefitWithCategories {
    #[serde(flatten)]
    pub benefit: Benefit,
    pub categories: Vec<Category>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Claim {
    pub id: String,
    pub status: String,
    pub reimbursement_status: Option<String>,
    pub payout_status: Option<String>,
    pub amount: Option<f64>,
    pub category: Option<String>,
    pub subcategory: Option<String>,
    pub reimbursement_vendor: Option<String>,
    pub date_processed: Option<String>,
    pub note: Option<String>,
    pub employee_note: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CreateClaimOptions {
    pub amount: String,
    pub merchant: String,
    pub purchase_date: String,
    pub description: String,
    pub receipt_path: Vec<PathBuf>,
    pub access_token: String,
    pub benefit_id: String,
    pub category_id: String,
    pub subcategory_value: String,
    pub subcategory_alias: Option<String>,
}

// ---------------------------------------------------------------------------
// Raw response schemas (only what we need)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct ProfileResponse {
    data: ProfileData,
}

#[derive(Debug, Deserialize)]
struct ProfileData {
    company: CompanyInfo,
    employee: EmployeeInfo,
}

#[derive(Debug, Deserialize)]
struct CompanyInfo {
    company_wallet_configurations: Vec<CompanyWalletConfiguration>,
}

#[derive(Debug, Deserialize)]
struct CompanyWalletConfiguration {
    wallet_name: String,
    categories: Vec<RawCategory>,
}

#[derive(Debug, Deserialize)]
struct RawCategory {
    id: String,
    name: String,
    subcategories: Vec<RawSubcategory>,
}

#[derive(Debug, Deserialize)]
struct RawSubcategory {
    name: String,
    value: String,
    #[serde(default)]
    aliases: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct EmployeeInfo {
    employee_wallets: Vec<EmployeeWallet>,
    settings: EmployeeSettings,
}

#[derive(Debug, Deserialize)]
struct EmployeeWallet {
    id: String,
    amount: f64,
    company_wallet_configuration: EmployeeWalletConfig,
    is_employee_eligible: bool,
}

#[derive(Debug, Deserialize)]
struct EmployeeWalletConfig {
    wallet_name: String,
}

#[derive(Debug, Deserialize)]
struct EmployeeSettings {
    currency: String,
}

#[derive(Debug, Deserialize)]
struct ClaimsListResponse {
    data: ClaimsListData,
}

#[derive(Debug, Deserialize)]
struct ClaimsListData {
    claims: Vec<RawClaim>,
    #[allow(dead_code)]
    page: Value,
    limit: serde_json::Value,
    count: u64,
}

#[derive(Debug, Deserialize)]
struct RawClaim {
    id: String,
    status: String,
    reimbursement: RawReimbursement,
}

#[derive(Debug, Deserialize)]
struct RawReimbursement {
    status: Option<String>,
    payout_status: Option<String>,
    amount: Option<f64>,
    category: Option<String>,
    subcategory: Option<String>,
    reimbursement_vendor: Option<String>,
    date_processed: Option<String>,
    note: Option<String>,
    employee_note: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GenericSuccessResponse {
    success: bool,
}

#[derive(Debug, Deserialize)]
struct MagicLinkExchangeResponse {
    success: bool,
    data: MagicLinkExchangeData,
}

#[derive(Debug, Deserialize)]
struct MagicLinkExchangeData {
    auth_token: String,
}

// ---------------------------------------------------------------------------
// Error handling
// ---------------------------------------------------------------------------

fn handle_error_response(response: Response) -> anyhow::Error {
    let status = response.status();
    let status_text = status
        .canonical_reason()
        .unwrap_or("unknown status")
        .to_string();
    let body = response.text().unwrap_or_default();

    if let Ok(parsed) = serde_json::from_str::<Value>(&body)
        && let Some(message) = parsed
            .get("errors")
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
    {
        if message.contains("JWT token is invalid") {
            return anyhow!(
                "Your Forma access token is invalid. Please log in again with `formanator login`."
            );
        }
        return anyhow!("{}", message);
    }

    anyhow!(
        "Received an unexpected {} {} response from Forma: {}",
        status.as_u16(),
        status_text,
        body
    )
}

// ---------------------------------------------------------------------------
// API operations
// ---------------------------------------------------------------------------

fn get_profile(access_token: &str) -> Result<ProfileResponse> {
    let response = client()?
        .get(format!("{API_BASE}/client/api/v3/settings/profile"))
        .header(AUTH_HEADER, access_token)
        .send()
        .context("Failed to call Forma profile endpoint")?;
    if !response.status().is_success() {
        return Err(handle_error_response(response));
    }
    response
        .json::<ProfileResponse>()
        .context("Failed to parse Forma profile response")
}

pub fn get_benefits(access_token: &str) -> Result<Vec<Benefit>> {
    let profile = get_profile(access_token)?;
    let currency = profile.data.employee.settings.currency.clone();

    Ok(profile
        .data
        .employee
        .employee_wallets
        .into_iter()
        .filter(|w| w.is_employee_eligible)
        .map(|w| Benefit {
            id: w.id,
            name: w.company_wallet_configuration.wallet_name,
            remaining_amount: w.amount,
            remaining_amount_currency: currency.clone(),
        })
        .collect())
}

pub fn get_categories_for_benefit_name(
    access_token: &str,
    benefit_name: &str,
) -> Result<Vec<Category>> {
    let profile = get_profile(access_token)?;

    let employee_wallet = profile
        .data
        .employee
        .employee_wallets
        .iter()
        .find(|w| {
            w.is_employee_eligible && w.company_wallet_configuration.wallet_name == benefit_name
        })
        .ok_or_else(|| anyhow!("Could not find benefit with name `{benefit_name}`."))?;

    let company_wallet = profile
        .data
        .company
        .company_wallet_configurations
        .iter()
        .find(|c| c.wallet_name == benefit_name)
        .ok_or_else(|| anyhow!("Could not find benefit with name `{benefit_name}`."))?;

    let benefit_id = employee_wallet.id.clone();

    let mut out = Vec::new();
    for category in &company_wallet.categories {
        for subcategory in &category.subcategories {
            out.push(Category {
                category_id: category.id.clone(),
                category_name: category.name.clone(),
                subcategory_name: subcategory.name.clone(),
                subcategory_value: subcategory.value.clone(),
                subcategory_alias: None,
                benefit_id: benefit_id.clone(),
            });
            for alias in &subcategory.aliases {
                out.push(Category {
                    category_id: category.id.clone(),
                    category_name: category.name.clone(),
                    subcategory_name: subcategory.name.clone(),
                    subcategory_value: subcategory.value.clone(),
                    subcategory_alias: Some(alias.clone()),
                    benefit_id: benefit_id.clone(),
                });
            }
        }
    }
    Ok(out)
}

pub fn get_benefits_with_categories(access_token: &str) -> Result<Vec<BenefitWithCategories>> {
    let benefits = get_benefits(access_token)?;
    let mut out = Vec::with_capacity(benefits.len());
    for benefit in benefits {
        let categories = get_categories_for_benefit_name(access_token, &benefit.name)?;
        out.push(BenefitWithCategories {
            benefit,
            categories,
        });
    }
    Ok(out)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimsFilter {
    InProgress,
}

fn fetch_claims_page(access_token: &str, page: u32) -> Result<ClaimsListData> {
    let url = format!("{API_BASE}/client/api/v2/claims?page={page}");
    let response = client()?
        .get(url)
        .header(AUTH_HEADER, access_token)
        .send()
        .context("Failed to call Forma claims endpoint")?;
    if !response.status().is_success() {
        return Err(handle_error_response(response));
    }
    let parsed: ClaimsListResponse = response
        .json()
        .context("Failed to parse Forma claims response")?;
    Ok(parsed.data)
}

pub fn get_claims_list(access_token: &str, filter: Option<ClaimsFilter>) -> Result<Vec<Claim>> {
    let mut all = Vec::new();
    let mut page = 0u32;
    loop {
        let data = fetch_claims_page(access_token, page)?;
        let limit_num: u64 = match &data.limit {
            Value::Number(n) => n.as_u64().unwrap_or(0),
            Value::String(s) => s.parse().unwrap_or(0),
            _ => 0,
        };
        let count = data.count;

        for raw in data.claims {
            all.push(Claim {
                id: raw.id,
                status: raw.status,
                reimbursement_status: raw.reimbursement.status,
                payout_status: raw.reimbursement.payout_status,
                amount: raw.reimbursement.amount,
                category: raw.reimbursement.category,
                subcategory: raw.reimbursement.subcategory,
                reimbursement_vendor: raw.reimbursement.reimbursement_vendor,
                date_processed: raw.reimbursement.date_processed,
                note: raw.reimbursement.note,
                employee_note: raw.reimbursement.employee_note,
            });
        }

        if limit_num == 0 || count != limit_num {
            break;
        }
        page += 1;
    }

    if filter == Some(ClaimsFilter::InProgress) {
        all.retain(|c| {
            c.status == "in_progress" || c.reimbursement_status.as_deref() == Some("in_progress")
        });
    }
    Ok(all)
}

pub fn create_claim(opts: &CreateClaimOptions) -> Result<()> {
    let mut form = multipart::Form::new()
        .text("type", "transaction".to_string())
        .text("is_recurring", "false".to_string())
        .text("amount", opts.amount.clone())
        .text("transaction_date", opts.purchase_date.clone())
        .text("default_employee_wallet_id", opts.benefit_id.clone())
        .text("note", opts.description.clone())
        .text("category", opts.category_id.clone())
        .text("category_alias", String::new())
        .text("subcategory", opts.subcategory_value.clone())
        .text(
            "subcategory_alias",
            opts.subcategory_alias.clone().unwrap_or_default(),
        )
        .text("reimbursement_vendor", opts.merchant.clone());

    for path in &opts.receipt_path {
        let abs: &Path = path.as_ref();
        form = form
            .file("file", abs)
            .with_context(|| format!("Failed to attach receipt at {}", abs.display()))?;
    }

    let response = client()?
        .post(format!("{API_BASE}/client/api/v2/claims"))
        .header(AUTH_HEADER, &opts.access_token)
        .multipart(form)
        .send()
        .context("Failed to submit claim to Forma")?;

    if response.status().as_u16() != 201 {
        return Err(handle_error_response(response));
    }

    let parsed: GenericSuccessResponse = response
        .json()
        .context("Failed to parse Forma claim creation response")?;
    if !parsed.success {
        bail!(
            "Something went wrong while submitting your claim. Forma returned `201 Created`, but the response body indicated that the request was not successful."
        );
    }
    Ok(())
}

/// Request a magic link be emailed to the user.
pub fn request_magic_link(email: &str) -> Result<()> {
    let response = client()?
        .post(format!("{API_BASE}/client/auth/v2/login/magic"))
        .json(&serde_json::json!({ "email": email }))
        .send()
        .context("Failed to request magic link")?;
    if !response.status().is_success() {
        return Err(handle_error_response(response));
    }
    let parsed: GenericSuccessResponse = response
        .json()
        .context("Failed to parse Forma magic link response")?;
    if !parsed.success {
        bail!("Something went wrong while requesting a magic link from Forma.");
    }
    Ok(())
}

/// Exchange a magic-link `id`/`tk` pair for a long-lived access token.
pub fn exchange_id_and_tk_for_access_token(id: &str, tk: &str) -> Result<String> {
    let response = client()?
        .get(format!("{API_BASE}/client/auth/v2/login/magic"))
        .query(&[("id", id), ("tk", tk), ("return_token", "true")])
        .send()
        .context("Failed to exchange magic link for an access token")?;
    if !response.status().is_success() {
        return Err(handle_error_response(response));
    }
    let parsed: MagicLinkExchangeResponse = response
        .json()
        .context("Failed to parse Forma magic link exchange response")?;
    if !parsed.success {
        bail!("Something went wrong while exchanging the magic link for an access token.");
    }
    Ok(parsed.data.auth_token)
}
