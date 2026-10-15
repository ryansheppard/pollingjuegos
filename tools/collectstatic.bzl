def _collectstatic_impl(ctx):
    output = ctx.actions.declare_directory(ctx.label.name)

    ctx.actions.run(
        executable = ctx.attr.tool[DefaultInfo].files_to_run,
        arguments = ["collectstatic", "--noinput"],
        outputs = [output],
        env = {
            "DJANGO_STATIC_ROOT": output.path,
        },
        mnemonic = "DjangCollectStatic",
    )

    return [DefaultInfo(files = depset([output]))]

collectstatic = rule(
    implementation = _collectstatic_impl,
    attrs = {
        "tool": attr.label(
            executable = True,
            cfg = "exec",
            mandatory = True,
        ),
    },
)
