# //:defs.bzl

load("@rules_rust//rust:defs.bzl", "rust_library", "rust_binary", "rust_test")
load("@rules_proto//proto:defs.bzl", "proto_library")
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
    proto_raw_name = name + "_raw_proto"

    native.proto_library(
        name = proto_raw_name,
        srcs = srcs,
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