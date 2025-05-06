# DAG Visualization Guide

This guide explains how to visualize the DAG (Directed Acyclic Graph) in ICN to better understand federation state and relationship between nodes.

## Basic Visualization

The ICN CLI provides built-in tools to generate visual representations of your federation's DAG:

```bash
icn dag visualize \
  --dag-dir ./dag-data \
  --output federation-dag.dot \
  --max-nodes 100
```

This generates a DOT format file which can be converted to various image formats:

```bash
# Generate PNG image
dot -Tpng federation-dag.dot -o federation-dag.png

# Generate SVG (for interactive viewing)
dot -Tsvg federation-dag.dot -o federation-dag.svg
```

## Filtering the DAG View

### By Author

To visualize only the nodes created by a specific author:

```bash
icn dag visualize \
  --dag-dir ./dag-data \
  --output author-dag.dot \
  --thread-did "did:example:specific-author"
```

### By Node Count

For large DAGs, limit the visualization to a manageable number of nodes:

```bash
icn dag visualize \
  --dag-dir ./dag-data \
  --output recent-dag.dot \
  --max-nodes 50
```

## Understanding the Visualization

The generated graph uses the following conventions:

- **Colors**:
  - Light Blue: TrustBundle nodes
  - Light Green: ExecutionReceipt nodes
  - Light Yellow: JSON payload nodes
  - White: Other node types

- **Node Labels**: Shows the node type, shortened CID, author, and timestamp

- **Edges**: Arrows point from child nodes to their parents, indicating the DAG relationship

## Advanced Visualization Tools

For more advanced analysis, you can use:

### Interactive DAG Explorer

For larger graphs, consider using interactive graph viewers:

```bash
# Using xdot for interactive viewing
xdot federation-dag.dot

# Using gephi for advanced graph analysis
# First convert DOT to GML format
dot -Tgml federation-dag.dot -o federation.gml
# Then import into Gephi
```

### Time-series Analysis

Visualize DAG evolution over time:

```bash
# Create snapshots at different timestamps
for cid in $(icn dag list-tips --dag-dir ./dag-data); do
  icn dag export-thread --from $CID --to $GENESIS_CID --dag-dir ./dag-data --output thread.json
  icn dag visualize --dag-dir ./dag-data --thread-json thread.json --output "dag-$CID.dot"
done
```

## Interpreting DAG Patterns

Different DAG structures indicate different federation behaviors:

- **Long Chains**: Sequential updates from a single authority
- **Wide Branching**: Many parallel operations (high activity)
- **Converging Branches**: Consensus being reached on disparate operations
- **Isolated Clusters**: Potential network partitions or independent activities

## Troubleshooting

- **Empty Graph**: Check that your DAG store contains nodes
- **Missing Connections**: Ensure you have all parent nodes synchronized
- **Too Complex**: Reduce the number of displayed nodes or filter by author 