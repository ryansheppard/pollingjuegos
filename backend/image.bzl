"""image_manifest that always builds its contents optimized."""

load("@rules_img//img:image.bzl", "image_manifest")
load("@with_cfg.bzl", "with_cfg")

# Without this, `bazel run //backend:push` ships whatever -c mode the invocation
# used, which defaults to an unoptimized binary with full debug info.
opt_image_manifest, _opt_image_manifest_internal = (
    with_cfg(image_manifest)
        .set("compilation_mode", "opt")
        .build()
)
