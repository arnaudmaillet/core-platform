use post::PostKernelCtx;
use shared_kernel::command::CommandBus;

pub struct PostCommandContainer {
    pub bus: CommandBus,
    pub kernel_ctx: PostKernelCtx,
}
