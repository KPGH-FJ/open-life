import { act, renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { TaskControl } from "@/tauri";
import { workbenchFixtureDataSource } from "@/test/fixtures/workbench/workbench";
import { useWorkbenchController } from "@/app/useWorkbenchController";

describe("useWorkbenchController", () => {
  it("refreshes dependent conversation state after a retry changes the canonical task", async () => {
    const fixture = workbenchFixtureDataSource("fixture-ready");
    const initial = await fixture.load();
    const taskId = initial.tasksEnvelope.data!.items[0].canonicalTaskId;
    const control: TaskControl = {
      id: `${taskId}:retry`,
      label: "重试失败步骤",
      kind: "retry",
      effect: "task_retry_request",
      enabled: true,
      requiresConfirmation: false,
      targetTaskId: taskId,
      targetActionId: "failed-web-search",
      completionProofAfterDispatch: false,
    };
    let retried = false;
    const dataSource = {
      ...fixture,
      async load() {
        const snapshot = await fixture.load();
        const current = snapshot.tasksEnvelope.data!.items[0];
        const task = {
          ...current,
          lifecycleStatus: retried ? ("completed" as const) : ("failed" as const),
          allowedControls: retried ? [] : [control],
        };
        return {
          ...snapshot,
          tasksEnvelope: {
            ...snapshot.tasksEnvelope,
            data: { ...snapshot.tasksEnvelope.data!, items: [task] },
          },
        };
      },
      async dispatchTaskControl(received: TaskControl) {
        expect(received).toEqual(control);
        retried = true;
      },
    };
    const announce = vi.fn();
    const refreshDependentState = vi.fn(async () => undefined);
    const { result } = renderHook(() =>
      useWorkbenchController(dataSource, announce, refreshDependentState)
    );

    await act(async () => {
      await result.current.load(false, "conversation-research-plan");
    });
    act(() => result.current.requestTaskControl(control, taskId));

    await waitFor(() => expect(refreshDependentState).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(result.current.taskControlState.phase).toBe("resolved"));
    expect(result.current.snapshot?.tasksEnvelope.data?.items[0].lifecycleStatus).toBe("completed");
  });

  it("re-reads task and conversation truth when retry reports an error after persistent mutation", async () => {
    const fixture = workbenchFixtureDataSource("fixture-ready");
    const initial = await fixture.load();
    const taskId = initial.tasksEnvelope.data!.items[0].canonicalTaskId;
    const control: TaskControl = {
      id: `${taskId}:retry-after-error`,
      label: "重试失败步骤",
      kind: "retry",
      effect: "task_retry_request",
      enabled: true,
      requiresConfirmation: false,
      targetTaskId: taskId,
      targetActionId: "failed-document-read",
      completionProofAfterDispatch: false,
    };
    let runCount = 1;
    const load = vi.fn(async () => {
      const snapshot = await fixture.load();
      const current = snapshot.tasksEnvelope.data!.items[0];
      return {
        ...snapshot,
        tasksEnvelope: {
          ...snapshot.tasksEnvelope,
          data: {
            ...snapshot.tasksEnvelope.data!,
            items: [{ ...current, runCount }],
          },
        },
      };
    });
    const dataSource = {
      ...fixture,
      load,
      async dispatchTaskControl(received: TaskControl) {
        expect(received).toEqual(control);
        runCount = 2;
        throw new Error("work_plan_required_step_incomplete");
      },
    };
    const announce = vi.fn();
    const refreshDependentState = vi.fn(async () => undefined);
    const { result } = renderHook(() =>
      useWorkbenchController(dataSource, announce, refreshDependentState)
    );

    await act(async () => {
      await result.current.load(false, "conversation-research-plan");
    });
    act(() => result.current.requestTaskControl(control, taskId));

    await waitFor(() => expect(result.current.taskControlState.phase).toBe("failed"));
    await waitFor(() => expect(refreshDependentState).toHaveBeenCalledTimes(1));
    expect(load).toHaveBeenCalledTimes(2);
    expect(announce).toHaveBeenCalledWith(
      "任务请求返回失败；任务与对话已重新读取，不会把错误回调解释成状态未改变。"
    );
  });
});
