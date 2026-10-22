// Called by actions/github-script after checking all artifact provenance.
module.exports = async ({ github, context, core }) => {
  const commit = process.env.RELEASE_COMMIT;
  if (!/^[a-f0-9]{40}$/.test(commit ?? "")) {
    throw new Error("Missing or invalid artifact source commit");
  }

  if (process.env.SKIP_CI === "true") {
    const reason = (process.env.CI_BYPASS_REASON ?? "").trim();
    if (context.eventName !== "workflow_dispatch" || !reason) {
      throw new Error(
        "A CI override requires a manual run and a non-empty reason",
      );
    }
    core.warning(`CI gate bypassed for ${commit}: ${reason}`);
    core.setOutput("evidence", `Manually bypassed for ${commit}: ${reason}`);
    await core.summary
      .addHeading("Release CI override")
      .addTable([
        [
          { data: "Commit", header: true },
          { data: "Actor", header: true },
        ],
        [commit, context.actor],
      ])
      .addCodeBlock(reason)
      .write();
    return;
  }

  const deadline = Date.now() + 30 * 60 * 1000;
  while (true) {
    const runs = await github.paginate(github.rest.actions.listWorkflowRuns, {
      ...context.repo,
      workflow_id: "ci.yml",
      head_sha: commit,
      branch: "main",
      per_page: 100,
    });
    // PR checks and runs for a different commit never authorize a release.
    const run = runs
      .filter(
        (candidate) =>
          candidate.head_sha === commit &&
          candidate.head_branch === "main" &&
          ["push", "workflow_dispatch", "schedule"].includes(candidate.event),
      )
      .sort((left, right) => right.id - left.id)[0];

    if (run?.status === "completed") {
      if (run.conclusion !== "success") {
        throw new Error(
          `CI for ${commit} ended with ${run.conclusion}: ${run.html_url}`,
        );
      }
      core.info(`CI passed for artifact source ${commit}: ${run.html_url}`);
      core.setOutput("evidence", run.html_url);
      await core.summary
        .addHeading("Release CI verified")
        .addLink(commit, run.html_url)
        .write();
      return;
    }

    if (Date.now() >= deadline) {
      throw new Error(
        `No successful main CI for ${commit} after 30 minutes. ` +
          "Run or rerun CI on that commit, then rerun Release server.",
      );
    }
    core.info(
      `Waiting for main CI on ${commit}: ${run?.status ?? "not found"}`,
    );
    await new Promise((resolve) => setTimeout(resolve, 20_000));
  }
};
