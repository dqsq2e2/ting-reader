
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

type TaskPayloadLabels = {
    libraryScan: (id: string) => string;
    metadataScrape: (query: string) => string;
    pluginInvoke: (pluginId: string, method: string) => string;
    formatConvert: (input: string, output: string) => string;
    writeMetadata: (bookId: string) => string;
    task: (taskType: string) => string;
    nodeLibraryScan: (id: string) => string;
};

type TaskStatusLabels = {
    completed: string;
    failed: string;
    running: string;
    cancelled: string;
    queued: string;
    unknown: string;
};

const defaultTaskPayloadLabels: TaskPayloadLabels = {
    libraryScan: (id) => `Library scan: ${id}`,
    metadataScrape: (query) => `Metadata scrape: ${query}`,
    pluginInvoke: (pluginId, method) => `Plugin call: ${pluginId} - ${method}`,
    formatConvert: (input, output) => `Format convert: ${input} -> ${output}`,
    writeMetadata: (bookId) => `Write metadata: ${bookId}`,
    task: (taskType) => `Task: ${taskType}`,
    nodeLibraryScan: (id) => `Library scan (ID: ${id.substring(0, 8)}...)`,
};

const defaultTaskStatusLabels: TaskStatusLabels = {
    completed: 'Completed',
    failed: 'Failed',
    running: 'Running',
    cancelled: 'Cancelled',
    queued: 'Queued',
    unknown: 'Unknown status',
};

export const formatTaskPayload = (
    payloadString: string,
    labels: TaskPayloadLabels = defaultTaskPayloadLabels,
): string => {
    if (!payloadString) return '';
    
    try {
        const payload = JSON.parse(payloadString) as TaskPayload;
        
        // Handle Rust backend format
        if (payload.Custom) {
            const { task_type, data } = payload.Custom;
            switch (task_type) {
                case 'library_scan':
                    return labels.libraryScan(data.library_id);
                case 'scraper_search':
                    return labels.metadataScrape(data.query);
                case 'plugin_invoke':
                    return labels.pluginInvoke(data.plugin_id, data.method);
                case 'format_convert':
                    return labels.formatConvert(data.input, data.output);
                case 'write_metadata':
                    return labels.writeMetadata(data.book_id);
                default:
                    return labels.task(task_type);
            }
        }
        
        // Handle Node.js backend format
        if (payload.library_id) {
            return labels.nodeLibraryScan(payload.library_id);
        }

        // Fallback for other JSON
        return payloadString;
    } catch {
        return payloadString;
    }
};

export const getTaskStatusText = (
    status: string,
    labels: TaskStatusLabels = defaultTaskStatusLabels,
): string => {
    switch (status) {
        case 'completed': return labels.completed;
        case 'failed': return labels.failed;
        case 'running': return labels.running;
        case 'cancelled': return labels.cancelled;
        case 'queued': return labels.queued;
        default: return labels.unknown;
    }
};
