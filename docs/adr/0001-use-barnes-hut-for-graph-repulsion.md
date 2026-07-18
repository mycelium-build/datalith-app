---
status: accepted
---

# Use Barnes-Hut approximation for graph repulsion

The Graph View must calculate repulsion repeatedly while animating as many as 10,000 nodes. We use a two-dimensional Barnes-Hut quadtree in [`src/ui/viewers/graph.rs`](../../src/ui/viewers/graph.rs) because it reduces the usual work from quadratic pairwise repulsion toward approximately `O(n log n)`, while its small force approximation is not perceptible enough to matter in an exploratory graph layout.

## How Barnes-Hut works

An exact repulsion calculation compares every node with every other node. Barnes-Hut instead partitions the two-dimensional scene into a quadtree: each occupied square can be divided into four smaller quadrants. Every quadtree cell records:

- its square bounds;
- its total mass, which is the number of contained graph nodes;
- the center of mass of those nodes;
- either one leaf body or up to four child cells.

To calculate the force on one graph node, the algorithm walks the tree from its root. A nearby cell is opened and its children are considered separately. A sufficiently distant cell is treated as one aggregate body located at its center of mass, so a single force calculation replaces all pairwise calculations against the nodes inside that cell.

The approximation uses the standard opening criterion `s / d < θ`, where `s` is the cell width and `d` is the distance from the graph node to the cell's center of mass. In this implementation, `θ` is `BARNES_HUT_THETA`. Smaller values inspect more cells and improve accuracy at greater cost; larger values aggregate more aggressively and run faster with a coarser approximation.

The cell containing the graph node is never approximated, which prevents its aggregate mass from applying self-repulsion. Leaf cells and accepted distant cells apply the configured repulsion strength using their aggregate mass. A softening term prevents an unbounded force at very short distances, and coincident positions receive a deterministic directional offset. Tree depth and minimum cell size are also bounded.

The quadtree is rebuilt for every simulation step because node positions change. Its scope is only the repulsive force: center and link forces are still calculated directly.

## Considered options

- **Exact all-pairs repulsion:** simplest and exact, but quadratic. At 10,000 nodes it requires tens of millions of node-pair interactions per simulation step, which is not compatible with an interactive animated view.
- **Spatial grid with a local cutoff:** efficient for short-range collision forces, but graph repulsion is intentionally long-range. A cutoff would introduce visible cell or radius boundaries and change the layout behavior.
- **Parallel all-pairs calculation:** could increase throughput but retains quadratic growth and adds platform, synchronization, and rendering integration complexity.

## Consequences

- Repulsion is approximate and `θ` is an accuracy/performance tuning parameter, not part of the Graph Definition format.
- Typical construction and force evaluation are approximately `O(n log n)` with `O(n)` storage, although pathological spatial distributions can perform worse.
- The approximation improves scalability but does not guarantee numerical stability by itself. Acceleration and velocity caps, initial-layout scaling, damping, softening, and endpoint-degree-normalized link forces remain necessary.
- The large-hub graph regression test must continue to verify finite positions and bounded velocity so future quadtree changes do not reintroduce node catapulting.
