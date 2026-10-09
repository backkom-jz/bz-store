import {test,expect,adminApi,makeCourse} from './support';
test('public catalog, preview and protected chapter',async({page})=>{
 const admin=await adminApi();const course=await makeCourse(admin);
 await page.goto('/');
 await expect(page.getByRole('heading',{name:'让技术更简单，让创意变现实。'})).toBeVisible();
 await page.goto('/tutorials/'+course.slug);
 await page.getByRole('button',{name:/01 免费预览/}).click();
 await expect(page.getByRole('heading',{name:'开始学习'})).toBeVisible();
 await page.getByRole('button',{name:/02 事务与授权/}).click();
 await expect(page.getByText('购买后解锁本章节')).toBeVisible();
 await expect(page.getByText('Paid browser-only secret')).toHaveCount(0);
 expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
 await admin.api.dispose();
});
test('registration return path is local and empty library is usable',async({page})=>{
 await page.goto('/register?returnTo='+encodeURIComponent('//evil.example'));
 await page.getByLabel('邮箱').fill('browser-'+crypto.randomUUID()+'@example.test');
 await page.getByLabel('密码',{exact:true}).fill('Browser-test-password-42');
 await page.getByRole('button',{name:'创建账号'}).click();
 await expect(page).toHaveURL('http://127.0.0.1:18088/');
 await page.goto('/user/library');await expect(page.getByText('还没有已购教程')).toBeVisible();
 await page.getByRole('link',{name:'去探索教程'}).click();await expect(page).toHaveURL(/\/tutorials$/);
});
