# //:defs.bzl

load("@rules_rust//rust:defs.bzl", "rust_library", "rust_binary", "rust_test")
load("@rules_proto//proto:defs.bzl", "proto_library")

# Puisqu'on a mis bazel_dep(name = "rules_rust_prost"), ce repo EXISTE forcément à la racine
load("@rules_rust_prost//:defs.bzl", "rust_prost_library")

DEFAULT_EDITION = "2024"

def core_rust_library(name, **kwargs):
    rust_library(
        name = name,
        edition = kwargs.pop("edition", DEFAULT_EDITION),
        **kwargs
    )

def core_rust_binary(name, **kwargs):
    rust_binary(
        name = name,
        edition = kwargs.pop("edition", DEFAULT_EDITION),
        **kwargs
    )

def core_rust_test(name, **kwargs):
    rust_test(
        name = name,
        edition = kwargs.pop("edition", DEFAULT_EDITION),
        **kwargs
    )

def core_rust_proto_library(name, srcs, deps = [], visibility = ["//visibility:public"]):
    proto_target_name = name + "_proto"

    proto_library(
        name = proto_target_name,
        srcs = srcs,
        # On remplace well_known_types par les cibles exactes de la v29
        deps = deps + [
            "@com_google_protobuf//:timestamp_proto",
            "@com_google_protobuf//:wrappers_proto",
        ],
        visibility = visibility,
    )

    rust_prost_library(
        name = name + "_rust_proto",
        proto = ":" + proto_target_name,
        visibility = visibility,
    )