use std::fs;
use std::path::Path;
use mars_data::mars::config::load_config;

// Helper function to create a temporary config file for testing
fn create_test_config_file(filename: &str, content: &str) -> Result<String, Box<dyn std::error::Error>> {
    let test_dir = Path::new("tests").join("temp");
    fs::create_dir_all(&test_dir)?;
    
    let file_path = test_dir.join(filename);
    fs::write(&file_path, content)?;
    
    Ok(file_path.to_string_lossy().into_owned())
}

// Helper function to clean up temporary test files
fn cleanup_test_file(file_path: &str) {
    let _ = fs::remove_file(file_path);
}

#[test]
fn test_load_config_valid_key() {
    // Create a temporary config file with valid DEMO_KEY
    let config_content = r#"
    {
      "DEMO_KEY": "TEST_API_KEY"
    }
    "#;
    
    let file_path = create_test_config_file("valid_config.json", config_content)
        .expect("Failed to create test config file");
    
    // Test loading the config
    let result = load_config(&file_path);
    
    // Clean up the test file
    cleanup_test_file(&file_path);
    
    // Verify the results
    assert!(result.is_ok(), "Config loading should succeed with valid JSON");
    let api_key = result.unwrap();
    assert_eq!(api_key, "TEST_API_KEY", "API key should match the one in config");
}

#[test]
fn test_load_config_missing_key() {
    // Create a temporary config file without DEMO_KEY
    let config_content = r#"
    {
      "OTHER_KEY": "some_value"
    }
    "#;
    
    let file_path = create_test_config_file("missing_key_config.json", config_content)
        .expect("Failed to create test config file");
    
    // Test loading the config
    let result = load_config(&file_path);
    
    // Clean up the test file
    cleanup_test_file(&file_path);
    
    // Verify the results
    assert!(result.is_err(), "Config loading should fail when DEMO_KEY is missing");
    let error = result.unwrap_err().to_string();
    assert!(error.contains("DEMO_KEY not found"), "Error message should mention missing key");
}

#[test]
fn test_load_config_non_string_key() {
    // Create a temporary config file with non-string DEMO_KEY
    let config_content = r#"
    {
      "DEMO_KEY": 12345
    }
    "#;
    
    let file_path = create_test_config_file("non_string_key_config.json", config_content)
        .expect("Failed to create test config file");
    
    // Test loading the config
    let result = load_config(&file_path);
    
    // Clean up the test file
    cleanup_test_file(&file_path);
    
    // Verify the results
    assert!(result.is_err(), "Config loading should fail when DEMO_KEY is not a string");
    let error = result.unwrap_err().to_string();
    assert!(error.contains("not a string"), "Error message should mention non-string value");
}

#[test]
fn test_load_config_invalid_json() {
    // Create a temporary config file with invalid JSON
    let config_content = r#"
    {
      "DEMO_KEY": "TEST_API_KEY"
      "invalid_json": true
    }
    "#;
    
    let file_path = create_test_config_file("invalid_json_config.json", config_content)
        .expect("Failed to create test config file");
    
    // Test loading the config
    let result = load_config(&file_path);
    
    // Clean up the test file
    cleanup_test_file(&file_path);
    
    // Verify the results
    assert!(result.is_err(), "Config loading should fail with invalid JSON");
}

#[test]
fn test_load_config_file_not_found() {
    // Test with a non-existent file
    let result = load_config("non_existent_config_file.json");
    
    // Verify the results
    assert!(result.is_err(), "Config loading should fail when file doesn't exist");
}
