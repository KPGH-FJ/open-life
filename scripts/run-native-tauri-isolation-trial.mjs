#!/usr/bin/env node

const fail = message => {
  throw new Error(message);
};
const assert = (condition, message) => {
  if (!condition) fail(message);
};

const servicePattern = /^com\.openlife\.desktop\.trial\.[0-9a-f]{32,}$/;

const parseArgs = values => {
  const parsed = {};
  for (const value of values) {
    assert(value.startsWith("--") && value.includes("="), `unsupported argument: ${value}`);
    const separator = value.indexOf("=");
    const key = value.slice(2, separator);
    assert(!(key in parsed), `duplicate argument: --${key}`);
    parsed[key] = value.slice(separator + 1);
  }
  return parsed;
};

const validatePreflight = args => {
  assert(args.mode === "preflight", "NTI-S1 permits preflight only; native execution belongs to NTI-S2");
  assert(args.profile === "debug", "native isolation trial requires a debug build");
  assert(args.features === "dev-extensions", "native isolation trial requires dev-extensions");
  assert(args.marker === "1", "native isolation trial marker must equal 1");
  assert(typeof args.service === "string" && args.service.length > 0, "trial Keychain service is required");
  assert(servicePattern.test(args.service), "trial Keychain service must use the isolated lowercase-hex namespace");
  assert(
    Object.keys(args).sort().join(",") === "features,marker,mode,profile,service",
    "preflight contains undeclared arguments"
  );
  return {
    outcome: "PASS",
    mode: "preflight",
    profile: "debug",
    features: ["dev-extensions"],
    service: args.service,
    external_actions: 0
  };
};

const runSelfTest = () => {
  const valid = {
    mode: "preflight",
    profile: "debug",
    features: "dev-extensions",
    marker: "1",
    service: "com.openlife.desktop.trial.0123456789abcdef0123456789abcdef"
  };
  validatePreflight(valid);
  const mutations = [
    value => delete value.service,
    value => delete value.marker,
    value => {
      value.marker = "true";
    },
    value => {
      value.profile = "release";
    },
    value => {
      value.features = "";
    },
    value => {
      value.service = "com.openlife.desktop";
    },
    value => {
      value.service = "com.openlife.desktop.trial.0123456789abcdef0123456789abcdeF";
    },
    value => {
      value.service = "com.openlife.desktop.trial.0123456789abcdef0123456789abcde";
    },
    value => {
      value.mode = "execute";
    },
    value => {
      value.unreviewed = "1";
    }
  ];
  for (const mutate of mutations) {
    const candidate = structuredClone(valid);
    mutate(candidate);
    let rejected = false;
    try {
      validatePreflight(candidate);
    } catch {
      rejected = true;
    }
    assert(rejected, "native isolation preflight accepted a counterexample");
  }
  const validArgs = [
    "--mode=preflight",
    "--profile=debug",
    "--features=dev-extensions",
    "--marker=1",
    `--service=${valid.service}`
  ];
  for (const key of ["mode", "profile", "features", "marker", "service"]) {
    let rejected = false;
    try {
      parseArgs([...validArgs, `--${key}=duplicate`]);
    } catch {
      rejected = true;
    }
    assert(rejected, `native isolation preflight accepted duplicate --${key}`);
  }
  const selfTestCount = mutations.length + 5;
  console.log(`Native Tauri isolation preflight self-test: PASS (${selfTestCount}/${selfTestCount})`);
};

if (process.argv.includes("--self-test")) {
  runSelfTest();
} else {
  const result = validatePreflight(parseArgs(process.argv.slice(2)));
  console.log(JSON.stringify(result));
}
