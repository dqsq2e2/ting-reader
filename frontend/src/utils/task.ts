
interface TaskPayload {
    Custom?: {
        task_type: string;
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        data: Record<string, any>;
    };
    libraryId?: string;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    [key: string]: any;
}

export const formatTaskPayload = (payloadString: string): string => {
    if (!payloadString) return '';
    
    try {
        const payload = JSON.parse(payloadString) as TaskPayload;
        
        // Handle Rust backend format
        if (payload.Custom) {
            const { task_type, data } = payload.Custom;
            switch (task_type) {
                case 'library_scan':
                    return `图书馆扫描: ${data.library_id}`; // Ideally we would map ID to name, but ID is what we have in payload
                case 'scraper_search':
                    return `元数据刮削: ${data.query}`;
                case 'plugin_invoke':
                    return `插件调用: ${data.plugin_id} - ${data.method}`;
                case 'format_convert':
                    return `格式转换: ${data.input} -> ${data.output}`;
                case 'write_metadata':
                    return `写入元数据: ${data.book_id}`;
                default:
                    return `任务: ${task_type}`;
            }
        }
        
        // Handle Node.js backend format
        if (payload.libraryId) {
            return `图书馆扫描 (ID: ${payload.libraryId.substring(0, 8)}...)`;
        }

        // Fallback for other JSON
        return payloadString;
    } catch {
        return payloadString;
    }
};

export const getTaskStatusText = (status: string): string => {
    switch (status) {
        case 'completed': return '已完成';
        case 'failed': return '失败';
        case 'running': return '进行中';
        case 'cancelled': return '已取消';
        case 'queued': return '等待中';
        default: return '未知状态';
    }
};
