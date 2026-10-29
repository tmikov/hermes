/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

package com.facebook.hermes.intlapp;

import android.app.Activity;
import android.os.Bundle;
import android.util.Log;
import android.widget.TextView;
import com.facebook.soloader.SoLoader;

/**
 * Loads Hermes through SoLoader, as React Native does, creates a runtime and
 * logs what the Intl plugin gives it (tag HermesIntlApp).
 */
public class MainActivity extends Activity {
  private static final String TAG = "HermesIntlApp";

  /** Create a Hermes runtime and return the Intl results, one per line. */
  private static native String nativeRun();

  @Override
  protected void onCreate(Bundle savedInstanceState) {
    super.onCreate(savedInstanceState);
    String result;
    try {
      SoLoader.init(this, false);
      SoLoader.loadLibrary("hermesintlapp");
      result = nativeRun();
    } catch (RuntimeException | UnsatisfiedLinkError e) {
      result = "error: " + e;
    }
    for (String line : result.split("\n")) {
      Log.i(TAG, line);
    }
    TextView view = new TextView(this);
    view.setText(result);
    setContentView(view);
  }
}
