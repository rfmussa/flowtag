import { describe, expect, it, vi } from 'vitest';
import { runQuitFlow, QuitDeps } from '@/lib/quit';

function deps(over: Partial<QuitDeps> = {}): QuitDeps {
    return {
        stageReady: 3,
        readyIds: [1, 2, 3],
        taggingRunning: false,
        confirm: vi.fn().mockResolvedValue('ok'),
        commitAndWait: vi.fn().mockResolvedValue(undefined),
        forceExit: vi.fn(),
        exitNormally: vi.fn(),
        ...over,
    };
}

describe('runQuitFlow', () => {
    it('commit_then_force_exit_when_confirmed', async () => {
        const d = deps();
        await runQuitFlow(d);
        expect(d.commitAndWait).toHaveBeenCalledWith([1, 2, 3]);
        expect(d.forceExit).toHaveBeenCalledOnce();
        expect(d.exitNormally).not.toHaveBeenCalled();
    });

    it('force_exits_even_when_the_commit_rejects', async () => {
        const d = deps({ commitAndWait: vi.fn().mockRejectedValue(new Error('socket closed')) });
        await runQuitFlow(d);
        expect(d.forceExit).toHaveBeenCalledOnce();
    });

    it('force_exits_when_the_commit_throws_synchronously', async () => {
        const d = deps({ commitAndWait: vi.fn(() => { throw new Error('boom'); }) });
        await runQuitFlow(d);
        expect(d.forceExit).toHaveBeenCalledOnce();
    });

    it('quit_anyway_falls_through_to_the_normal_exit', async () => {
        const d = deps({ confirm: vi.fn().mockResolvedValue('cancel') });
        await runQuitFlow(d);
        expect(d.commitAndWait).not.toHaveBeenCalled();
        expect(d.exitNormally).toHaveBeenCalledOnce();
        expect(d.forceExit).not.toHaveBeenCalled();
    });

    it('dismissed_stays_in_the_app', async () => {
        const d = deps({ confirm: vi.fn().mockResolvedValue('dismiss') });
        await runQuitFlow(d);
        expect(d.commitAndWait).not.toHaveBeenCalled();
        expect(d.exitNormally).not.toHaveBeenCalled();
        expect(d.forceExit).not.toHaveBeenCalled();
    });

    it('skips_the_offer_while_tagging_runs', async () => {
        const d = deps({ taggingRunning: true });
        await runQuitFlow(d);
        expect(d.confirm).not.toHaveBeenCalled();
        expect(d.exitNormally).toHaveBeenCalledOnce();
    });

    it('nothing_ready_goes_straight_to_the_normal_exit', async () => {
        const d = deps({ stageReady: 0, readyIds: [] });
        await runQuitFlow(d);
        expect(d.confirm).not.toHaveBeenCalled();
        expect(d.exitNormally).toHaveBeenCalledOnce();
    });
});
