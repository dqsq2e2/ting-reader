import React, { useEffect, useState } from 'react';
import apiClient from '../api/client';
import { Trash2, HardDrive, Download, Database, ChevronDown, ChevronRight } from 'lucide-react';
import { useNavigate } from 'react-router-dom';

const DownloadsPage: React.FC = () => {
  const navigate = useNavigate();
  const [cachedFiles, setCachedFiles] = useState<any[]>([]);
  const [loading, setLoading] = useState(true);
  const [expandedBookTitle, setExpandedBookTitle] = useState<string | null>(null);

  useEffect(() => {
    fetchCachedFiles();
  }, []);

  const fetchCachedFiles = async () => {
    try {
      const res = await apiClient.get('/api/cache');
      setCachedFiles(res.data);
    } catch (err) {
      console.error('Failed to fetch cache:', err);
    } finally {
      setLoading(false);
    }
  };

  const handleClearAll = async () => {
    if (!confirm('确定要清空所有服务端缓存吗？这将删除所有已缓存的文件。')) return;
    
    try {
      await apiClient.delete('/api/cache');
      setCachedFiles([]);
    } catch (err) {
      console.error('Failed to clear cache:', err);
    }
  };

  const handleDelete = async (id: string) => {
    if (!confirm('确定要删除此缓存文件吗？')) return;
    
    try {
      await apiClient.delete(`/api/cache/${id}`);
      setCachedFiles(prev => prev.filter(f => f.id !== id));
    } catch (err) {
      console.error('Failed to delete cache:', err);
    }
  };

  const formatSize = (bytes: number) => {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
  };

  // Group files by book
  const bookGroups = React.useMemo(() => {
    const groups: Record<string, { title: string, count: number, size: number, coverUrl?: string, files: typeof cachedFiles }> = {};
    
    cachedFiles.forEach(file => {
      const bookTitle = file.bookTitle || '未知书籍';
      if (!groups[bookTitle]) {
        groups[bookTitle] = {
          title: bookTitle,
          count: 0,
          size: 0,
          coverUrl: file.coverUrl,
          files: []
        };
      }
      groups[bookTitle].count++;
      groups[bookTitle].size += file.size;
      groups[bookTitle].files.push(file);
    });
    
    return Object.values(groups);
  }, [cachedFiles]);

  const toggleExpand = (title: string) => {
    if (expandedBookTitle === title) {
      setExpandedBookTitle(null);
    } else {
      setExpandedBookTitle(title);
    }
  };

  const handleDeleteBook = async (bookTitle: string, e: React.MouseEvent) => {
    e.stopPropagation();
    const filesToDelete = cachedFiles.filter(f => (f.bookTitle || '未知书籍') === bookTitle);
    
    if (!confirm(`确定要删除《${bookTitle}》的所有缓存吗？(${filesToDelete.length} 章)`)) return;

    for (const file of filesToDelete) {
      try {
        await apiClient.delete(`/api/cache/${file.id}`);
      } catch (err) {
        console.error(`Failed to delete cache ${file.id}`, err);
      }
    }
    
    setCachedFiles(prev => prev.filter(f => (f.bookTitle || '未知书籍') !== bookTitle));
  };

  return (
    <div className="flex-1 min-h-full flex flex-col p-4 sm:p-6 md:p-8 animate-in fade-in duration-500">
      <div className="flex items-center justify-between mb-8">
        <div>
            <div className="flex items-center gap-3">
                <h1 className="text-2xl md:text-3xl font-bold dark:text-white flex items-center gap-3">
                    <Download size={28} className="text-primary-600 md:w-8 md:h-8" />
                    缓存管理
                </h1>
            </div>
            <p className="text-sm md:text-base text-slate-500 mt-1 ml-10">
                管理服务端缓存文件（WebDAV 等非本地资源）。开启自动缓存后，播放时将自动缓存文件以加速访问。
            </p>
        </div>
        <div className="flex gap-2">
            {cachedFiles.length > 0 && (
                <button 
                    onClick={handleClearAll} 
                    className="flex items-center gap-2 bg-slate-100 text-slate-600 hover:bg-slate-200 dark:bg-slate-800 dark:text-slate-400 dark:hover:bg-slate-700 px-4 py-2 rounded-xl transition-colors font-medium text-sm"
                >
                    <Trash2 size={18} />
                    清空缓存
                </button>
            )}
        </div>
      </div>

      <div className="bg-white dark:bg-slate-900 rounded-2xl border border-slate-100 dark:border-slate-800 shadow-sm overflow-hidden">
        {loading ? (
             <div className="flex items-center justify-center p-12">
                <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-primary-600"></div>
             </div>
        ) : cachedFiles.length === 0 ? (
            <div className="text-center py-20 px-4">
                <div className="inline-flex items-center justify-center w-16 h-16 rounded-full bg-slate-50 dark:bg-slate-800 text-slate-400 mb-4">
                    <Database size={32} />
                </div>
                <h3 className="text-lg font-bold text-slate-900 dark:text-white mb-1">暂无缓存文件</h3>
                <p className="text-slate-500 text-sm max-w-xs mx-auto">当用户播放或预加载音频时，服务端会自动缓存文件以提高访问速度。</p>
            </div>
        ) : (
            <div className="divide-y divide-slate-100 dark:divide-slate-800">
                {bookGroups.map(group => (
                    <div key={group.title} className="bg-white dark:bg-slate-900 transition-colors">
                        {/* Book Header */}
                        <div 
                            className="p-4 flex items-center gap-4 hover:bg-slate-50 dark:hover:bg-slate-800/50 cursor-pointer group"
                            onClick={() => toggleExpand(group.title)}
                        >
                            <div className="w-8 h-8 flex items-center justify-center text-slate-400">
                                {expandedBookTitle === group.title ? <ChevronDown size={20} /> : <ChevronRight size={20} />}
                            </div>

                            <div className="w-10 h-14 bg-slate-200 dark:bg-slate-700 rounded-md shrink-0 flex items-center justify-center overflow-hidden">
                                {group.coverUrl ? (
                                    <img src={group.coverUrl} alt={group.title} className="w-full h-full object-cover" />
                                ) : (
                                    <Download size={20} className="text-slate-400" />
                                )}
                            </div>

                            <div className="flex-1 min-w-0">
                                <h3 className="font-bold text-slate-900 dark:text-white truncate">{group.title}</h3>
                                <div className="flex items-center gap-2 text-sm text-slate-500 mt-1">
                                    <span>{group.count} 章</span>
                                    <span>•</span>
                                    <span>{formatSize(group.size)}</span>
                                </div>
                            </div>
                            
                            <button
                                onClick={(e) => handleDeleteBook(group.title, e)}
                                className="p-2 text-slate-400 hover:text-red-500 hover:bg-red-50 dark:hover:bg-red-900/20 rounded-lg transition-colors opacity-0 group-hover:opacity-100 focus:opacity-100"
                                title="删除整书缓存"
                            >
                                <Trash2 size={18} />
                            </button>
                        </div>

                        {/* Chapters List (Accordion Content) */}
                        {expandedBookTitle === group.title && (
                            <div className="bg-slate-50/50 dark:bg-slate-800/20 border-t border-slate-100 dark:border-slate-800 divide-y divide-slate-100 dark:divide-slate-800 pl-4 sm:pl-12">
                                {group.files.map(file => (
                                    <div key={file.id} className="p-3 pl-4 hover:bg-slate-100/50 dark:hover:bg-slate-800/50 transition-colors flex items-center gap-4 group/item">
                                        {/* Icon Status */}
                                        <div className="w-8 h-8 rounded-lg flex items-center justify-center shrink-0 bg-blue-50 text-blue-500 dark:bg-blue-900/20">
                                            <HardDrive size={16} />
                                        </div>

                                        {/* Content */}
                                        <div className="flex-1 min-w-0">
                                            <div className="flex items-center gap-2">
                                                <h4 className="font-medium text-slate-900 dark:text-white truncate text-sm">{file.title || `Chapter ${file.id}`}</h4>
                                                <span className="text-[10px] bg-slate-100 dark:bg-slate-800 px-1.5 py-0.5 rounded-full text-slate-500">
                                                    {formatSize(file.size)}
                                                </span>
                                            </div>
                                            <div className="flex items-center gap-3 text-[10px] text-slate-400 mt-0.5">
                                                <span>缓存时间: {new Date(file.mtime).toLocaleString()}</span>
                                            </div>
                                        </div>

                                        {/* Actions */}
                                        <button 
                                            onClick={(e) => { e.stopPropagation(); handleDelete(file.id); }}
                                            className="p-2 text-slate-400 hover:text-red-500 hover:bg-red-50 dark:hover:bg-red-900/20 rounded-lg transition-all opacity-0 group-hover/item:opacity-100 focus:opacity-100"
                                            title="删除缓存"
                                        >
                                            <Trash2 size={16} />
                                        </button>
                                    </div>
                                ))}
                            </div>
                        )}
                    </div>
                ))}
            </div>
        )}
      </div>
    </div>
  );
};

export default DownloadsPage;