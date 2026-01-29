# //:defs.bzl

load("@rules_rust//rust:defs.bzl", "rust_library", "rust_binary", "rust_test")
load("@rules_proto//proto:defs.bzl", "proto_library")
load("@rules_rust_prost//:defs.bzl", "rust_prost_library")
load("@rules_oci//oci:defs.bzl", "oci_image", "oci_push")
load("@aspect_bazel_lib//lib:tar.bzl", "tar")

DEFAULT_EDITION = "2024"
ECR_REGISTRY = "724772065879.dkr.ecr.us-east-1.amazonaws.com"

def core_rust_library(name, **kwargs):
    rust_library(
        name = name,
        edition = kwargs.pop("edition", DEFAULT_EDITION),
        **kwargs
    )

def core_rust_binary(name, **kwargs):
    # 1. On récupère la visibilité pour la transmettre aux cibles OCI
    vis = kwargs.get("visibility", ["//visibility:public"])

    # 2. On garde la compilation Rust d'origine
    rust_binary(
        name = name,
        edition = kwargs.pop("edition", DEFAULT_EDITION),
        **kwargs
    )

    # 3. Packaging : On met le binaire dans un layer tar
    tar(
        name = name + "_layer",
        srcs = [":" + name],
        visibility = ["//visibility:private"],
    )

    # 4. Image OCI : Construction de l'image distroless
    oci_image(
        name = name + "_image",
        base = "@distroless_rust",
        # ATTENTION : Bazel place souvent le binaire à la racine ou dans un chemin relatif au package
        # Si ton binaire s'appelle 'account-service', l'entrypoint sera /account-service
        entrypoint = ["/" + name],
        tars = [":" + name + "_layer"],
        visibility = vis,
    )

    # 5. Push : Commande pour envoyer vers ton ECR
    native.genrule(
        name = name + "_tags",
        outs = [name + "_tags.txt"],
        cmd = "echo 'latest' > $@ && grep 'STABLE_GIT_SHA' bazel-out/stable-status.txt | cut -d ' ' -f 2 >> $@",
        stamp = 1,
    )

    oci_push(
        name = name + "_push",
        image = ":" + name + "_image",
        repository = ECR_REGISTRY + "/core-platform-" + name.replace("_", "-"),
        remote_tags = ":" + name + "_tags",
    )

def core_rust_test(name, **kwargs):
    rust_test(
        name = name,
        edition = kwargs.pop("edition", DEFAULT_EDITION),
        **kwargs
    )

def core_rust_proto_library(name, srcs, deps = [], visibility = ["//visibility:public"]):
    proto_raw_name = name + "_raw_proto"

    native.proto_library(
        name = proto_raw_name,
        srcs = srcs,
        strip_import_prefix = "/proto",
        deps = deps + [
            "@com_google_protobuf//:timestamp_proto",
            "@com_google_protobuf//:wrappers_proto",
        ],
        visibility = ["//visibility:private"],
    )

    rust_prost_library(
        name = name,
        proto = ":" + proto_raw_name,
        visibility = visibility,
    )