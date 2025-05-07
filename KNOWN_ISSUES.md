# Known Issues - Post Merge Tasks

This document tracks issues that are known but not blocking for the merge to main. These should be addressed in follow-up work.

## ICN-Economics Crate Issues

### RwLock Usage Compatibility
- **Description**: The `icn-economics` crate has some compatibility issues with RwLock usage between async and sync contexts.
- **Impact**: Not critical as the economics crate is not directly used by the observability tools.
- **Proposed Solution**: Refactor to use a consistent approach to RwLock throughout the codebase.
- **Priority**: Medium
- **Estimated Effort**: 2-3 days

## Testing Limitations

### Unit Tests Not Running
- **Description**: Unit tests for several modules exist but cannot run due to dependency issues.
- **Impact**: Reduces confidence in code quality, though manual testing confirms functionality.
- **Proposed Solution**: Fix dependency issues and expand test coverage.
- **Priority**: High
- **Estimated Effort**: 3-5 days

### Integration Tests Needed
- **Description**: Need comprehensive integration tests that verify system-wide functionality.
- **Impact**: Makes it harder to verify all components work together correctly.
- **Proposed Solution**: Create integration test suite with CI/CD pipeline integration.
- **Priority**: High
- **Estimated Effort**: 5-7 days

## Performance Considerations

### DAG Performance with Large Networks
- **Description**: DAG operations may slow down with very large networks.
- **Impact**: Could affect usability in production deployments with many nodes.
- **Proposed Solution**: Implement indexing and pagination for DAG operations.
- **Priority**: Medium
- **Estimated Effort**: 3-4 days

### UI Rendering Optimization
- **Description**: Rendering large DAGs in the UI can be slow.
- **Impact**: Affects user experience when viewing large DAGs.
- **Proposed Solution**: Implement progressive loading and virtualization for large datasets.
- **Priority**: Medium
- **Estimated Effort**: 2-3 days

## Usability Enhancements

### Mobile UX Improvements
- **Description**: While the UI is responsive, some interactions could be optimized for mobile.
- **Impact**: Slightly degraded experience on mobile devices.
- **Proposed Solution**: Mobile-specific UI optimizations and touch interactions.
- **Priority**: Low
- **Estimated Effort**: 2-3 days

### Documentation Expansion
- **Description**: Documentation is functional but could be expanded with more examples.
- **Impact**: Learning curve for new users might be steeper than necessary.
- **Proposed Solution**: Add more examples, tutorials, and use cases to documentation.
- **Priority**: Medium
- **Estimated Effort**: 2-3 days

## Feature Requests (Not Issues)

### Enhanced Analytics Dashboard
- **Description**: Add more analytics and visualizations to the dashboard.
- **Priority**: Low
- **Estimated Effort**: 5-7 days

### Governance Simulation Tools
- **Description**: Add tools to simulate governance actions and their effects.
- **Priority**: Low
- **Estimated Effort**: 4-5 days 