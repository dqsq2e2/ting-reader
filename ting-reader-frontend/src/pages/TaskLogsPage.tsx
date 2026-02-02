import React, { useEffect, useState } from 'react';
import apiClient from '../api/client';
import { 
  Terminal, 
  RefreshCw, 
  CheckCircle2, 
  XCircle, 
  Clock, 
  Loader2,
  Database,
  Search
} from 'lucide-react';

interface Task {
  id: string;
  type: string;
  status: 'pending' | 'processing' | 'completed' | 'failed';
  payload: string;
  message?: string;
  error?: string;
  created_at: string;
  updated_at: string;
}

const TaskLogsPage: React.FC = () => {
  const [tasks, setTasks] = useState<Task[]>([]);
  const [loading, setLoading] = useState(true);
  const [autoRefresh, setAutoRefresh] = useState(true);

  useEffect(() => {
    fetchTasks();
    let interval: ReturnType<typeof setInterval>;
    if (autoRefresh) {
      interval = setInterval(fetchTasks, 3000);
    }
    return () => clearInterval(interval);
  }, [autoRefresh]);

  const fetchTasks = async () => {
    try {
      const response = await apiClient.get('/api/tasks');
      setTasks(response.data);
    } catch (err) {
      console.error('Failed to fetch tasks', err);
    } finally {
      setLoading(false);
    }
  };

  const getStatusIcon = (status: Task['status']) => {
    switch (status) {
      case 'completed': return <CheckCircle2 className="text-green-500" size={20} />;
      case 'failed': return <XCircle className="text-red-500" size={20} />;
      case 'processing': return <Loader2 className="text-blue-500 animate-spin" size={20} />;
      default: return <Clock className="text-slate-400" size={20} />;
    }
  };

  const getStatusText = (status: Task['status']) => {
    switch (status) {
      case 'completed': return '已完成';
      case 'failed': return '失败';
      case 'processing': return '进行中';
      default: return '等待中';
    }
  };

  const parsePayload = (payload: string) => {
    try {
      const data = JSON.parse(payload);
      return data.libraryId ? `扫描库 ID: ${data.libraryId.substring(0, 8)}...` : payload;
    } catch {
      return payload;
    }
  };

  if (loading && tasks.length === 0) {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-primary-600"></div>
      </div>
    );
  }

  return (
    <div className="w-full max-w-screen-2xl mx-auto p-4 sm:p-6 md:p-8 lg:p-10 space-y-8">
      <div className="flex flex-col md:flex-row md:items-center justify-between gap-6">
        <div className="text-center md:text-left">
          <h1 className="text-2xl md:text-3xl font-bold dark:text-white flex items-center justify-center md:justify-start gap-3">
            <Terminal size={28} className="text-primary-600 md:w-8 md:h-8" />
            任务日志
          </h1>
          <p className="text-sm md:text-base text-slate-500 mt-1">实时监控系统扫描与刮削进度</p>
        </div>
        <div className="flex items-center justify-center gap-4 bg-white dark:bg-slate-900 p-3 rounded-2xl border border-slate-100 dark:border-slate-800 shadow-sm md:shadow-none md:border-none md:p-0 md:bg-transparent">
          <div className="flex items-center gap-2 text-sm text-slate-500">
            <span className="font-bold md:font-normal">自动刷新</span>
            <button
              onClick={() => setAutoRefresh(!autoRefresh)}
              className={`w-12 h-6 rounded-full transition-all relative ${
                autoRefresh ? 'bg-primary-600' : 'bg-slate-200 dark:bg-slate-700'
              }`}
            >
              <div className={`absolute top-1 w-4 h-4 bg-white rounded-full transition-all ${
                autoRefresh ? 'left-7' : 'left-1'
              }`} />
            </button>
          </div>
          <button 
            onClick={fetchTasks}
            className="p-2.5 bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl text-slate-600 dark:text-slate-400 hover:bg-slate-50 transition-colors shadow-sm"
          >
            <RefreshCw size={20} className={loading ? 'animate-spin' : ''} />
          </button>
        </div>
      </div>

      <div className="bg-white dark:bg-slate-900 rounded-3xl overflow-hidden border border-slate-100 dark:border-slate-800 shadow-sm">
        <div className="divide-y divide-slate-100 dark:divide-slate-800">
          {tasks.map((task) => (
            <div key={task.id} className="p-6 hover:bg-slate-50/50 dark:hover:bg-slate-800/30 transition-colors">
              <div className="flex items-start justify-between gap-4">
                <div className="flex items-start gap-4">
                  <div className={`w-12 h-12 rounded-xl flex items-center justify-center ${
                    task.type === 'scan' ? 'bg-blue-50 text-blue-600 dark:bg-blue-900/20' : 'bg-purple-50 text-purple-600 dark:bg-purple-900/20'
                  }`}>
                    {task.type === 'scan' ? <Database size={24} /> : <Search size={24} />}
                  </div>
                  <div>
                    <div className="flex items-center gap-3">
                      <span className="font-bold dark:text-white">
                        {task.type === 'scan' ? '库扫描任务' : '刮削任务'}
                      </span>
                      <span className={`text-[10px] font-bold uppercase tracking-widest px-2 py-0.5 rounded-md ${
                        task.status === 'completed' ? 'bg-green-100 text-green-600 dark:bg-green-900/20 dark:text-green-400' :
                        task.status === 'failed' ? 'bg-red-100 text-red-600 dark:bg-red-900/20 dark:text-red-400' :
                        'bg-blue-100 text-blue-600 dark:bg-blue-900/20 dark:text-blue-400'
                      }`}>
                        {getStatusText(task.status)}
                      </span>
                    </div>
                    <p className="text-sm text-slate-500 mt-1">{parsePayload(task.payload)}</p>
                    {task.message && (
                      <p className="text-sm font-medium text-primary-600 dark:text-primary-400 mt-2 flex items-center gap-2">
                        <Loader2 size={14} className={task.status === 'processing' ? 'animate-spin' : ''} />
                        {task.message}
                      </p>
                    )}
                    {task.error && (
                      <p className="text-xs text-red-500 mt-2 bg-red-50 dark:bg-red-900/10 p-2 rounded-lg border border-red-100 dark:border-red-900/20">
                        错误: {task.error}
                      </p>
                    )}
                  </div>
                </div>
                <div className="text-right shrink-0">
                  <div className="flex items-center justify-end gap-2 mb-1">
                    {getStatusIcon(task.status)}
                  </div>
                  <div className="text-xs text-slate-400">
                    {new Date(task.created_at).toLocaleString()}
                  </div>
                </div>
              </div>
            </div>
          ))}
        </div>
        
        {tasks.length === 0 && (
          <div className="py-20 text-center">
            <Terminal size={48} className="mx-auto text-slate-200 mb-4" />
            <p className="text-slate-500 font-medium">暂无任务日志</p>
          </div>
        )}
      </div>
    </div>
  );
};

export default TaskLogsPage;
