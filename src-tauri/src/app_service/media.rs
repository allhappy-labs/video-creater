use super::error::ServiceError;

pub struct MediaService;

impl MediaService {
    pub fn validate_display_name(name: &str) -> Result<&str, ServiceError> {
        let name = name.trim();
        if name.is_empty()
            || name.len() > 255
            || name.contains('/')
            || name.contains('\\')
            || name.chars().any(char::is_control)
        {
            return Err(ServiceError::invalid_input("media display name is invalid"));
        }
        Ok(name)
    }
}
