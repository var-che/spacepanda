#[cfg(test)]
mod test_import {
    #[test]
    fn can_import_test_harness() {
        // This test just verifies the module can be imported
        use spacepanda_core::core_mvp::test_harness;
        let _ = test_harness::start_server;
    }
}
