use aws_sdk_dynamodb::model::AttributeValue;
use std::collections::HashMap;

pub trait AttributeValuesExt {
    fn get_string(&self, key: &str) -> Option<String>;
    fn get_number(&self, key: &str) -> Option<u64>;
    fn get_bool(&self, key: &str) -> Option<bool>;
    fn get_array_string(&self, key: &str) -> Option<Vec<String>>;
}

impl AttributeValuesExt for HashMap<String, AttributeValue> {
    fn get_array_string(&self, key: &str) -> Option<Vec<String>> {
        let results = self.get(key)?.as_l().ok()?;
        let mut results_vec: Vec<String> = Vec::new();
        for result in results {
            let item = result.as_s().ok()?;
            results_vec.push(item.clone());
        }

        Some(results_vec)
    }

    fn get_string(&self, key: &str) -> Option<String> {
        Some(self.get(key)?.as_s().ok()?.to_owned())
    }

    fn get_number(&self, key: &str) -> Option<u64> {
        self.get(key)?.as_n().ok()?.parse().ok()
    }

    fn get_bool(&self, key: &str) -> Option<bool> {
        Some(self.get(key)?.as_bool().ok()?.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Result;

    #[test]
    fn dynamodb_get_string() -> Result<()> {
        // ARRANGE
        let mut value = HashMap::new();
        value.insert("pk".to_string(), AttributeValue::S("something".to_string()));

        // ACT
        let result = value.get_string("pk").unwrap();

        // ASSERT
        assert_eq!("something".to_string(), result);
        Ok(())
    }

    #[test]
    fn dynamodb_get_bool() -> Result<()> {
        // ARRANGE
        let mut value = HashMap::new();
        value.insert("my-bool".to_string(), AttributeValue::Bool(true));

        // ACT
        let result = value.get_bool("my-bool").unwrap();

        // ASSERT
        assert_eq!(true, result);
        Ok(())
    }

    #[test]
    fn dynamodb_get_number() -> Result<()> {
        // ARRANGE
        let mut value = HashMap::new();
        value.insert("my-number".to_string(), AttributeValue::N("1".to_string()));

        // ACT
        let result = value.get_number("my-number").unwrap();

        // ASSERT
        assert_eq!(1, result);
        Ok(())
    }
}
