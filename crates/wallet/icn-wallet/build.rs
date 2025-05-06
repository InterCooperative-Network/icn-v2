fn main() {
    #[cfg(feature = "uniffi-bindings")]
    {
        uniffi::generate_scaffolding("src/icn-wallet.udl").unwrap();
    }
} 