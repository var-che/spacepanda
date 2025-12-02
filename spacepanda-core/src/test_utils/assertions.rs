//! Custom assertions and matchers for tests
//!
//! Provides expressive assertion helpers that improve test readability
//! and provide better error messages.

use std::fmt::Debug;

/// Assert that a Result is Ok and return the value
pub fn assert_ok<T, E: Debug>(result: Result<T, E>) -> T {
    match result {
        Ok(value) => value,
        Err(e) => panic!("Expected Ok, got Err: {:?}", e),
    }
}

/// Assert that a Result is Err and return the error
pub fn assert_err<T: Debug, E>(result: Result<T, E>) -> E {
    match result {
        Ok(value) => panic!("Expected Err, got Ok: {:?}", value),
        Err(e) => e,
    }
}

/// Assert that an Option is Some and return the value
pub fn assert_some<T>(option: Option<T>) -> T {
    match option {
        Some(value) => value,
        None => panic!("Expected Some, got None"),
    }
}

/// Assert that an Option is None
pub fn assert_none<T: Debug>(option: Option<T>) {
    if let Some(value) = option {
        panic!("Expected None, got Some({:?})", value);
    }
}

/// Assert that a collection contains an element
pub fn assert_contains<T: PartialEq + Debug>(collection: &[T], element: &T) {
    if !collection.contains(element) {
        panic!(
            "Expected collection to contain {:?}, but it didn't. Collection: {:?}",
            element, collection
        );
    }
}

/// Assert that a collection does not contain an element
pub fn assert_not_contains<T: PartialEq + Debug>(collection: &[T], element: &T) {
    if collection.contains(element) {
        panic!(
            "Expected collection not to contain {:?}, but it did. Collection: {:?}",
            element, collection
        );
    }
}

/// Assert that two collections have the same elements (order doesn't matter)
pub fn assert_same_elements<T: PartialEq + Debug>(a: &[T], b: &[T]) {
    if a.len() != b.len() {
        panic!(
            "Collections have different lengths: {} vs {}. a: {:?}, b: {:?}",
            a.len(),
            b.len(),
            a,
            b
        );
    }
    for item in a {
        if !b.contains(item) {
            panic!(
                "Element {:?} from first collection not found in second. a: {:?}, b: {:?}",
                item, a, b
            );
        }
    }
}

/// Assert that a value is within a range
pub fn assert_in_range<T: PartialOrd + Debug>(value: T, min: T, max: T) {
    if value < min || value > max {
        panic!(
            "Value {:?} is not in range [{:?}, {:?}]",
            value, min, max
        );
    }
}

/// Assert that a future completes within a timeout
#[macro_export]
macro_rules! assert_timeout {
    ($duration:expr, $future:expr) => {{
        match tokio::time::timeout($duration, $future).await {
            Ok(result) => result,
            Err(_) => panic!("Future did not complete within {:?}", $duration),
        }
    }};
}

/// Assert that a future times out
#[macro_export]
macro_rules! assert_does_timeout {
    ($duration:expr, $future:expr) => {{
        match tokio::time::timeout($duration, $future).await {
            Ok(_) => panic!("Expected future to timeout, but it completed"),
            Err(_) => (),
        }
    }};
}

/// Assert that two floating point numbers are approximately equal
pub fn assert_approx_eq(a: f64, b: f64, epsilon: f64) {
    if (a - b).abs() > epsilon {
        panic!(
            "Values are not approximately equal: {} vs {} (epsilon: {})",
            a, b, epsilon
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_assert_ok() {
        let result: Result<i32, &str> = Ok(42);
        assert_eq!(assert_ok(result), 42);
    }

    #[test]
    #[should_panic(expected = "Expected Ok, got Err")]
    fn test_assert_ok_panics_on_err() {
        let result: Result<i32, &str> = Err("error");
        let _ = assert_ok(result);
    }

    #[test]
    fn test_assert_err() {
        let result: Result<i32, &str> = Err("error");
        assert_eq!(assert_err(result), "error");
    }

    #[test]
    #[should_panic(expected = "Expected Err, got Ok")]
    fn test_assert_err_panics_on_ok() {
        let result: Result<i32, &str> = Ok(42);
        let _ = assert_err(result);
    }

    #[test]
    fn test_assert_some() {
        let option = Some(42);
        assert_eq!(assert_some(option), 42);
    }

    #[test]
    #[should_panic(expected = "Expected Some, got None")]
    fn test_assert_some_panics_on_none() {
        let option: Option<i32> = None;
        let _ = assert_some(option);
    }

    #[test]
    fn test_assert_none() {
        let option: Option<i32> = None;
        assert_none(option);
    }

    #[test]
    #[should_panic(expected = "Expected None, got Some")]
    fn test_assert_none_panics_on_some() {
        let option = Some(42);
        assert_none(option);
    }

    #[test]
    fn test_assert_contains() {
        let vec = vec![1, 2, 3];
        assert_contains(&vec, &2);
    }

    #[test]
    #[should_panic(expected = "Expected collection to contain")]
    fn test_assert_contains_panics_when_missing() {
        let vec = vec![1, 2, 3];
        assert_contains(&vec, &4);
    }

    #[test]
    fn test_assert_in_range() {
        assert_in_range(5, 1, 10);
    }

    #[test]
    #[should_panic(expected = "not in range")]
    fn test_assert_in_range_panics_outside() {
        assert_in_range(15, 1, 10);
    }

    #[test]
    fn test_assert_approx_eq() {
        assert_approx_eq(1.0, 1.0001, 0.001);
    }

    #[test]
    #[should_panic(expected = "not approximately equal")]
    fn test_assert_approx_eq_panics_when_different() {
        assert_approx_eq(1.0, 2.0, 0.001);
    }
}
