## 介绍

rbackup 是一个简单的上传s3、ssh服务器的文件备份工具.

### 使用方法

![image-20240618145521603](https://yanbingmd.oss-cn-chengdu.aliyuncs.com/md/image-20240618145521603.png)

```shell
rbackup start # 直接备份
rbackup schedule # 将备份命令写入系统定时
rbackup clear # 清理定时
rbackup -h # 查询命令参数
```

示例

![image-20240618145958533](https://yanbingmd.oss-cn-chengdu.aliyuncs.com/md/image-20240618145958533.png)

```shell
rbackup   start  -s config.toml.teplate
```

### 已支持

- [x] 上传minio
- [x] 上传腾讯云
- [ ] 上传阿里云
- [x] 目录、规则上传