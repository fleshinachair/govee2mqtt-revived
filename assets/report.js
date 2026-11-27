// The "Report a problem" page. Text from the report is only ever assigned with
// textContent or as a URL parameter, never as HTML.

const ISSUES = "https://github.com/fleshinachair/govee2mqtt-revived/issues/new";

const output = document.getElementById("report");
const models = document.getElementById("model");
const download = document.getElementById("download");
const issue = document.getElementById("issue");

function updateIssueLink(version) {
  const url = new URL(ISSUES);
  url.searchParams.set("template", "report.yml");
  url.searchParams.set("version", version);
  if (models.value) {
    url.searchParams.set("sku", models.value);
    url.searchParams.set("title", `${models.value}: `);
  }
  issue.href = url.toString();
}

async function load() {
  const response = await fetch("../api/diagnostics");
  if (!response.ok) {
    throw new Error(`status ${response.status}`);
  }
  const report = await response.json();
  const text = JSON.stringify(report, null, 2);
  output.textContent = text;

  for (const model of new Set(report.devices.map((d) => d.model))) {
    const option = document.createElement("option");
    option.value = model;
    option.textContent = model;
    models.append(option);
  }

  download.href = URL.createObjectURL(
    new Blob([text], { type: "application/json" }),
  );
  download.download = "govee2mqtt-report.json";
  download.classList.remove("disabled");

  models.addEventListener("change", () => updateIssueLink(report.version));
  updateIssueLink(report.version);
}

load().catch((error) => {
  output.textContent = `The report could not be built: ${error.message}`;
});
