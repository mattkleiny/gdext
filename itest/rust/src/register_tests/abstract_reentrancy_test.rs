/*
 * Copyright (c) godot-rust; Bromeon and contributors.
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

//! Re-entrancy and cross-level dispatch for Rust base classes.
//!
//! Members inherited from a Rust base class are registered for the subclass with a projection into the embedded base instance. Every call
//! that re-enters the object takes a *fresh* borrow of the subclass's cell, so recursion, nested binds and dispatch back up the chain all
//! stress the same mechanism. These tests pin which combinations work and which are rejected.

use godot::builtin::Array;
use godot::classes::{ClassDb, Node, Object, RefCounted};
use godot::obj::NewAlloc;
use godot::prelude::*;

use crate::framework::{expect_panic, itest, suppress_godot_print, suppress_panic_log};

// ----------------------------------------------------------------------------------------------------------------------------------------------
// Composite hierarchy: recursive dispatch across many objects.

#[derive(GodotClass)]
#[class(abstract, base = RefCounted)]
pub struct TreeNode {
    base: Base<RefCounted>,

    #[var]
    label: GString,
}

#[godot_api]
impl TreeNode {
    #[func(abstract)]
    fn weight(&self) -> i64;

    /// Template method: formats the subclass's recursive result.
    #[func]
    fn described_weight(&self) -> GString {
        GString::from(&format!("{}={}", self.label, self.weight()))
    }

    /// Concrete base helper, reachable from subclasses both in Rust and through Godot.
    #[func]
    fn label_len(&self) -> i64 {
        self.label.len() as i64
    }
}

#[derive(GodotClass)]
#[class(init, base = TreeNode)]
pub struct TreeLeaf {
    tree: TreeNode,

    #[var]
    value: i64,
}

#[godot_api]
impl TreeLeaf {}

#[godot_api]
impl ITreeNode for TreeLeaf {
    fn weight(&self) -> i64 {
        self.value
    }
}

#[derive(GodotClass)]
#[class(init, base = TreeNode)]
pub struct TreeBranch {
    tree: TreeNode,

    #[var]
    children: Array<Gd<TreeNode>>,
}

#[godot_api]
impl TreeBranch {
    #[func]
    fn add_child_node(&mut self, child: Gd<TreeNode>) {
        self.children.push(&child);
    }
}

#[godot_api]
impl ITreeNode for TreeBranch {
    /// Recursion: each child's `weight()` re-enters Godot and binds *that* object anew.
    fn weight(&self) -> i64 {
        self.children
            .iter_shared()
            .map(|child| child.upcast::<Object>().call("weight", &[]).to::<i64>())
            .sum()
    }
}

// ----------------------------------------------------------------------------------------------------------------------------------------------
// Mutual recursion between a base method and the subclass's implementation, on a *single* object.

#[derive(GodotClass)]
#[class(abstract, base = RefCounted)]
pub struct RecurseBase {
    base: Base<RefCounted>,

    #[var]
    depth_seen: i64,
}

#[godot_api]
impl RecurseBase {
    #[func(abstract)]
    fn step(&self, n: i64) -> i64;

    /// Calls back into the subclass, which calls this again -- one nested borrow of the same object per level.
    #[func]
    fn countdown(&self, n: i64) -> i64 {
        if n <= 0 { 0 } else { 1 + self.step(n - 1) }
    }
}

#[derive(GodotClass)]
#[class(init, base = RecurseBase)]
pub struct RecurseUser {
    recurse: RecurseBase,
}

#[godot_api]
impl RecurseUser {}

#[godot_api]
impl IRecurseBase for RecurseUser {
    fn step(&self, n: i64) -> i64 {
        // Direct Rust call on the embedded base: no Godot round-trip, no new borrow.
        self.recurse.countdown(n)
    }
}

// ----------------------------------------------------------------------------------------------------------------------------------------------
// Dispatch back up a three-level chain, from Rust and through Godot.

#[derive(GodotClass)]
#[class(abstract, base = Node)]
pub struct ChainTop {
    base: Base<Node>,

    #[var]
    #[init(val = 100)]
    top: i64,
}

#[godot_api]
impl ChainTop {
    #[func]
    fn at_top(&self) -> i64 {
        self.top
    }

    #[func(abstract)]
    fn hook(&self) -> i64;
}

#[derive(GodotClass)]
#[class(abstract, base = ChainTop)]
pub struct ChainMid {
    chain_top: ChainTop,

    #[var]
    #[init(val = 20)]
    mid: i64,
}

#[godot_api]
impl ChainMid {
    #[func]
    fn at_mid(&self) -> i64 {
        // Reaches the grandparent's concrete method directly in Rust.
        self.mid + self.chain_top.at_top()
    }
}

#[derive(GodotClass)]
#[class(init, base = ChainMid)]
pub struct ChainLeaf {
    chain_mid: ChainMid,

    #[var]
    #[init(val = 3)]
    leaf: i64,
}

#[godot_api]
impl ChainLeaf {
    /// Calls both ancestors' methods directly in Rust.
    #[func]
    fn sum_rust(&self) -> i64 {
        self.leaf + self.chain_mid.at_mid()
    }

    /// Calls the same ancestors' methods through Godot, i.e. through the registered projections.
    #[func]
    fn sum_godot(&self) -> i64 {
        let mut object = godot::obj::WithBaseField::to_gd(self).upcast::<Object>();
        self.leaf + object.call("at_mid", &[]).to::<i64>() + object.call("at_top", &[]).to::<i64>()
    }
}

#[godot_api]
impl IChainTop for ChainLeaf {
    /// A "super"-style call: the abstract implementation delegates to a concrete base method.
    fn hook(&self) -> i64 {
        self.chain_mid.at_mid() * 2
    }
}

// ----------------------------------------------------------------------------------------------------------------------------------------------
// Signals on the base, connected to a method of the *same* object.

#[derive(GodotClass)]
#[class(abstract, base = RefCounted)]
pub struct EmitBase {
    base: Base<RefCounted>,

    #[var]
    emitted: i64,
}

#[godot_api]
impl EmitBase {
    #[signal]
    fn pinged(value: i64);

    #[func]
    fn ping(&mut self, value: i64) {
        self.emitted += 1;
        self.signals().pinged().emit(value);
    }
}

#[derive(GodotClass)]
#[class(init, base = EmitBase)]
pub struct EmitUser {
    emit: EmitBase,

    #[var]
    received: i64,
}

#[godot_api]
impl EmitUser {
    /// Handler on the same object, taking `&mut self`.
    #[func]
    fn on_pinged(&mut self, value: i64) {
        self.received += value;
    }
}

// ----------------------------------------------------------------------------------------------------------------------------------------------
// Tests: recursive dispatch across objects

#[itest]
fn recursive_dispatch_across_tree() {
    let mut root = TreeBranch::new_gd();
    let mut inner = TreeBranch::new_gd();

    for value in [1, 2, 3] {
        let mut leaf = TreeLeaf::new_gd();
        leaf.bind_mut().value = value;
        inner.bind_mut().add_child_node(leaf.upcast());
    }

    let mut deep_leaf = TreeLeaf::new_gd();
    deep_leaf.bind_mut().value = 10;
    inner.bind_mut().add_child_node(deep_leaf.upcast());

    root.bind_mut().add_child_node(inner.clone().upcast());

    let mut extra = TreeLeaf::new_gd();
    extra.bind_mut().value = 100;
    root.bind_mut().add_child_node(extra.upcast());

    // Three levels of recursion, each step re-entering Godot and binding a different object.
    let total = root
        .clone()
        .upcast::<Object>()
        .call("weight", &[])
        .to::<i64>();
    assert_eq!(total, 1 + 2 + 3 + 10 + 100);

    // The base's template method sits on top of that recursion.
    root.bind_mut().tree.label = "root".into();
    let described = root
        .upcast::<Object>()
        .call("described_weight", &[])
        .to::<GString>();
    assert_eq!(described, "root=116");
}

#[itest]
fn recursive_dispatch_mixed_subclasses() {
    // A branch whose children are of mixed concrete types: each child's projection differs.
    let mut root = TreeBranch::new_gd();

    let mut leaf = TreeLeaf::new_gd();
    leaf.bind_mut().value = 7;
    root.bind_mut().add_child_node(leaf.upcast());

    let empty_branch = TreeBranch::new_gd();
    root.bind_mut().add_child_node(empty_branch.upcast());

    assert_eq!(root.upcast::<Object>().call("weight", &[]).to::<i64>(), 7);
}

#[itest]
fn nested_binds_same_object() {
    let mut user = RecurseUser::new_gd();
    let mut object = user.clone().upcast::<Object>();

    // `countdown(5)` alternates base -> subclass -> base ..., re-entering the *same* object five times. Each round-trip takes a fresh
    // shared borrow while the outer ones are still alive.
    assert_eq!(object.call("countdown", &[5.to_variant()]).to::<i64>(), 5);
    assert_eq!(object.call("countdown", &[0.to_variant()]).to::<i64>(), 0);

    // Same from Rust, with an outer shared borrow already held.
    assert_eq!(user.bind().recurse.countdown(3), 3);

    // Deeper, to make sure nothing is capped at a small borrow count.
    assert_eq!(object.call("countdown", &[20.to_variant()]).to::<i64>(), 20);

    user.bind_mut().recurse.depth_seen = 1; // still usable afterwards
    assert_eq!(user.bind().recurse.depth_seen, 1);
}

#[itest]
fn concurrent_shared_binds_two_handles() {
    let first = RecurseUser::new_gd();
    let second = first.clone();

    // Two handles to the same object, both bound shared at once.
    let guard_a = first.bind();
    let guard_b = second.bind();

    assert_eq!(guard_a.recurse.depth_seen, guard_b.recurse.depth_seen);
    drop((guard_a, guard_b));
}

#[itest]
fn mut_bind_blocks_nested_shared_bind() {
    let mut first = RecurseUser::new_gd();
    let second = first.clone();

    let _guard = first.bind_mut();

    // gdext's borrow rules are unchanged by the embedding: an exclusive borrow still excludes a shared one.
    expect_panic("shared bind while mut-bound", move || {
        let _ = second.bind();
    });
}

#[itest]
fn template_method_rejected_from_mut_context() {
    let mut user = RecurseUser::new_gd();

    // The forwarder needs a shared borrow of the object; an accessible `&mut` borrow excludes it.
    expect_panic("forwarder under &mut borrow", move || {
        let guard = user.bind_mut();
        let _ = guard.recurse.countdown(1);
    });
}

// ----------------------------------------------------------------------------------------------------------------------------------------------
// Tests: dispatch back up the chain

#[itest]
fn dispatch_up_chain_rust_and_godot() {
    let leaf = ChainLeaf::new_alloc();
    let mut object = leaf.clone().upcast::<Object>();

    // 3 + (20 + 100)
    assert_eq!(object.call("sum_rust", &[]).to::<i64>(), 123);

    // Same numbers, but each ancestor method goes through its registered projection.
    assert_eq!(object.call("sum_godot", &[]).to::<i64>(), 3 + 120 + 100);

    // Abstract hook implemented by delegating to a concrete base method.
    assert_eq!(object.call("hook", &[]).to::<i64>(), 240);

    leaf.free();
}

#[itest]
fn dispatch_up_chain_through_parent_handle() {
    let leaf = ChainLeaf::new_alloc();

    // Hold the object as each ancestor type and call methods that live at *other* levels.
    let as_mid: Gd<ChainMid> = leaf.clone().upcast();
    let as_top: Gd<ChainTop> = leaf.clone().upcast();

    let mut via_mid = as_mid.upcast::<Object>();
    assert_eq!(via_mid.call("at_top", &[]).to::<i64>(), 100);
    assert_eq!(
        via_mid.call("sum_rust", &[]).to::<i64>(),
        123,
        "leaf method"
    );

    let mut via_top = as_top.upcast::<Object>();
    assert_eq!(via_top.call("at_mid", &[]).to::<i64>(), 120);
    assert_eq!(via_top.call("hook", &[]).to::<i64>(), 240);

    leaf.free();
}

#[itest]
fn chain_state_shared_between_levels() {
    let mut leaf = ChainLeaf::new_alloc();
    let mut object = leaf.clone().upcast::<Object>();

    // Writing a grandparent property through Godot is seen by a parent method reading it in Rust.
    object.set("top", &5.to_variant());
    assert_eq!(object.call("at_mid", &[]).to::<i64>(), 25);
    assert_eq!(leaf.bind().chain_mid.chain_top.top, 5);

    // ...and the other way round.
    leaf.bind_mut().chain_mid.mid = 1;
    assert_eq!(object.call("at_mid", &[]).to::<i64>(), 6);

    leaf.free();
}

#[itest]
fn base_mut_escape_hatch_from_subclass() {
    // A *concrete* subclass can take the re-borrow guard, which is what makes re-entrant calls possible from a `&mut self` context.
    let mut leaf = ChainLeaf::new_alloc();

    {
        let mut guard = leaf.bind_mut();
        let mut base = godot::obj::WithBaseField::base_mut(&mut *guard);
        assert_eq!(base.call("at_top", &[]).to::<i64>(), 100);
        assert_eq!(base.call("hook", &[]).to::<i64>(), 240);
    }

    leaf.free();
}

// ----------------------------------------------------------------------------------------------------------------------------------------------
// Tests: signals emitted from a base method

#[itest]
fn base_signal_to_closure_handler() {
    let user = EmitUser::new_gd();
    let mut object = user.clone().upcast::<Object>();

    let seen = Array::<i64>::new();
    let seen_clone = seen.clone();
    let callable = Callable::from_fn("seen", move |args: &[&Variant]| {
        seen_clone.clone().push(args[0].to::<i64>());
        Variant::nil()
    });
    object.connect("pinged", &callable);

    // Emitting from a base method reaches a handler that does not borrow the object.
    object.call("ping", &[9.to_variant()]);

    assert_eq!(seen.len(), 1);
    assert_eq!(seen.at(0), 9);
    assert_eq!(user.bind().emit.emitted, 1);
}

#[itest]
fn base_signal_to_same_object_handler_rejected() {
    let user = EmitUser::new_gd();
    let mut object = user.clone().upcast::<Object>();

    let callable = Callable::from_object_method(&user, "on_pinged");
    object.connect("pinged", &callable);

    // The handler takes `&mut self` on the object that `ping()` already borrows mutably. An abstract class cannot hand out the re-borrow
    // guard that would allow it, so the handler's invocation is refused at the FFI boundary: gdext reports an error there and the handler
    // does not run, rather than reinterpreting the instance. `ping()` itself still completes.
    suppress_panic_log(|| {
        suppress_godot_print(|| {
            object.call("ping", &[1.to_variant()]);
        })
    });

    assert_eq!(user.bind().emit.emitted, 1, "ping() itself completed");
    assert_eq!(user.bind().received, 0, "handler did not run");
}

#[itest]
fn base_signal_registered_once() {
    let class_db = ClassDb::singleton();

    // Signals stay on the declaring class; the subclass inherits them through ClassDB rather than re-registering.
    assert!(class_db.class_has_signal("EmitBase", "pinged"));
    assert!(class_db.class_has_signal("EmitUser", "pinged"));

    let own_signals = class_db
        .class_get_signal_list_ex("EmitUser")
        .no_inheritance(true)
        .done();
    assert!(
        !own_signals
            .iter_shared()
            .any(|dict| dict.at("name").to::<GString>() == "pinged")
    );
}
