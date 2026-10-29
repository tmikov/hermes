/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include "hermes/VM/RuntimeFlags.h"

namespace hermes::cli {

#ifndef HERMES_IS_MOBILE_BUILD
/// Build runtime config from the parsed command line flags.
vm::RuntimeConfig buildRuntimeConfig(const RuntimeFlags &flags) {
  vm::RuntimeConfig::Builder builder;
  builder
      .withGCConfig(vm::GCConfig::Builder()
                        .withInitHeapSize(flags.InitHeapSize.bytes)
                        .withMaxHeapSize(flags.MaxHeapSize.bytes)
                        .withOccupancyTarget(flags.OccupancyTarget)
                        .withSanitizeConfig(
                            vm::GCSanitizeConfig::Builder()
                                .withSanitizeRate(flags.GCSanitizeRate)
                                .withRandomSeed(flags.GCSanitizeRandomSeed)
                                .build())
                        .withShouldRecordStats(flags.GCPrintStats)
                        .withShouldReleaseUnused(vm::kReleaseUnusedOld)
                        .withAllocInYoung(flags.GCAllocYoung)
                        .withRevertToYGAtTTI(flags.GCRevertToYGAtTTI)
                        .build())
      .withMaxNumRegisters(flags.MaxNumRegisters)
      .withEnableEval(flags.EnableEval)
      .withVerifyEvalIR(flags.VerifyIR)
      .withOptimizedEval(flags.OptimizedEval)
      .withAsyncBreakCheckInEval(flags.EmitAsyncBreakCheck)
      .withVMExperimentFlags(flags.VMExperimentFlags)
      .withES6Proxy(flags.ES6Proxy)
      .withIntl(flags.Intl)
      .withMicrotaskQueue(flags.MicrotaskQueue)
      .withEnableSampleProfiling(
          flags.SampleProfiling != ExecuteOptions::SampleProfilingMode::None)
      .withRandomizeMemoryLayout(flags.RandomizeMemoryLayout)
      .withTrackIO(flags.TrackBytecodeIO)
      .withEnableHermesInternal(flags.EnableHermesInternal)
      .withEnableHermesInternalTestMethods(
          flags.EnableHermesInternalTestMethods)
      .withFinalizerThreadRunner(toolFinalizerThreadRunner());
  return withIntlPluginFlags(builder, flags).build();
}
#endif // HERMES_IS_MOBILE_BUILD

vm::RuntimeConfig::Builder &withIntlPluginFlags(
    vm::RuntimeConfig::Builder &builder,
    const VMOnlyRuntimeFlags &flags) {
  return builder.withEnableIntlPlugin(flags.IntlPlugin)
      .withIntlPluginPath(flags.IntlPluginPath)
      .withIntlDefaultLocales(std::vector<std::string>(
          flags.IntlDefaultLocales.begin(), flags.IntlDefaultLocales.end()))
      .withIntlDefaultTimeZone(flags.IntlDefaultTimeZone);
}

} // namespace hermes::cli
