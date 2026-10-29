# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

module HermesHelper
  # BUILD_TYPE = :debug
  BUILD_TYPE = :release
end

Pod::Spec.new do |spec|
  spec.name        = "hermes-engine"
  # This must be consistent with the release_version in:
  # - android/build.gradle
  # - npm/package.json
  # - CMakeLists.txt
  spec.version     = "0.12.0"
  spec.summary     = "Hermes is a small and lightweight JavaScript engine optimized for running React Native."
  spec.description = "Hermes is a JavaScript engine optimized for fast start-up of React Native apps. It features ahead-of-time static optimization and compact bytecode."
  spec.homepage    = "https://hermesengine.dev"
  spec.license     = { type: "MIT", file: "LICENSE" }
  spec.author      = "Facebook"
  # This env var should be supplied with a CDN URL of the "hermes-runtime-darwin.tgz" on the Github releases before pod push.
  # The podspec would be serialized to JSON and people will download prebuilt binaries instead of the source.
  # TODO(use the hash field as a validation mechanism when the process is stable)
  spec.source      = ENV['hermes-artifact-url'] ? { http: ENV['hermes-artifact-url'] } : { git: "https://github.com/facebook/hermes.git", tag: "v#{spec.version}" }
  # The deployment targets the prebuilt frameworks are built with (CI sets
  # IOS_DEPLOYMENT_TARGET 15.1, MAC_DEPLOYMENT_TARGET 10.15 and
  # XROS_DEPLOYMENT_TARGET 1.0; tvOS uses the iOS value, see
  # utils/build-apple-framework.sh).
  spec.platforms   = { :osx => "10.15", :ios => "15.1", :visionos => "1.0", :tvos => "15.1" }

  spec.preserve_paths      = ["destroot/bin/*"].concat(HermesHelper::BUILD_TYPE == :debug ? ["**/*.{h,c,cpp}"] : [])
  spec.source_files        = "destroot/include/**/*.h"
  spec.header_mappings_dir = "destroot/include"

  # Hermes and its Intl plugin. The app embeds both; Hermes finds the plugin
  # next to hermesvm.framework when it creates a runtime, and uses its
  # built-in Intl when the plugin is absent.
  frameworks = [
    "destroot/Library/Frameworks/universal/hermesvm.xcframework",
    "destroot/Library/Frameworks/universal/hermesintl.xcframework",
  ]
  spec.ios.vendored_frameworks = frameworks
  spec.visionos.vendored_frameworks = frameworks
  spec.tvos.vendored_frameworks = frameworks
  spec.osx.vendored_frameworks = frameworks

  spec.xcconfig            = { "CLANG_CXX_LANGUAGE_STANDARD" => "c++17", "CLANG_CXX_LIBRARY" => "compiler-default", "GCC_PREPROCESSOR_DEFINITIONS" => "HERMES_ENABLE_DEBUGGER=1" }

  # CocoaPods links every vendored framework, but nothing in the app refers to
  # the plugin: Hermes loads it with dlopen. The link is weak, so the app
  # still launches when the plugin is missing: removed from the app, or
  # without the app's architecture. The tvOS and visionOS simulator plugins
  # are arm64 only, but their xcframework slices list x86_64 and have
  # hermesvm's slice names (utils/intl-xcframework-sim-slices.py), so an
  # x86_64 simulator build copies and embeds them; the linker ignores them
  # for x86_64, and an x86_64 process finds no usable plugin, which Hermes
  # treats as no plugin.
  spec.user_target_xcconfig = {
    "OTHER_LDFLAGS" => "$(inherited) -weak_framework hermesintl",
  }

  unless ENV['hermes-artifact-url']
    spec.prepare_command = <<-EOS
      # When true, debug build will be used.
      # See `build-apple-framework.sh` for details
      DEBUG=#{HermesHelper::BUILD_TYPE == :debug}

      # Build all Apple frameworks (iOS, macOS, tvOS, visionOS, Catalyst)
      ./utils/build-apple-framework.sh
    EOS
  end
end
