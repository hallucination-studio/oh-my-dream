use assets::asset::interfaces::{
    AssetClockInterface, AssetIdentityGeneratorInterface, AssetIngestTransactionInterface,
    AssetManagedContentStoreInterface, AssetMediaInspectorInterface, AssetRepositoryInterface,
};

#[test]
fn asset_interfaces_are_object_safe_send_and_sync_boundaries() {
    fn assert_boundary<T: ?Sized + Send + Sync>() {}

    assert_boundary::<dyn AssetRepositoryInterface>();
    assert_boundary::<dyn AssetIngestTransactionInterface>();
    assert_boundary::<dyn AssetManagedContentStoreInterface>();
    assert_boundary::<dyn AssetMediaInspectorInterface>();
    assert_boundary::<dyn AssetClockInterface>();
    assert_boundary::<dyn AssetIdentityGeneratorInterface>();
}
