# fixed_capacity_vec

Extend Vec to allow reference to content while pushing new elements.

Inspired by [this Pre-RFC](https://internals.rust-lang.org/t/pre-rfc-fixed-capacity-view-of-vec/8413)

## Example

```rust
use fixed_capacity_vec::AsFixedCapacityVec;

let mut vec = vec![1, 2, 3, 4];
{
    let (mut content, mut extend_end) = vec.with_fixed_capacity(5);
    extend_end.push(4);
    assert_eq!(extend_end.as_ref(), &[4]);

    // We can still access content here.
    assert_eq!(content, &[1, 2, 3, 4]);

    // We can even copy one buffer into the other
    extend_end.extend_from_slice(content);
    assert_eq!(extend_end.as_ref(), &[4, 1, 2, 3, 4]);

    // The following line would panic because we reached max. capacity:
    // extend_end.push(10);
}
// All operations happened on vec
assert_eq!(vec, &[1, 2, 3, 4, 4, 1, 2, 3, 4]);
```
