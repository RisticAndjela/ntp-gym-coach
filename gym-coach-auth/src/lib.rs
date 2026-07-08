pub mod app;

pub use app::run;

pub fn validate_register_input(full_name: &str, email: &str, password: &str) -> Result<(), &'static str> {
    if full_name.trim().len() < 2 {
        return Err("Full name must contain at least 2 characters");
    }

    let normalized_email = email.trim();
    if normalized_email.is_empty() || !normalized_email.contains('@') {
        return Err("Email address is not valid");
    }

    if password.len() < 6 {
        return Err("Password must contain at least 6 characters");
    }

    Ok(())
}

