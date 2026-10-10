import {test,expect,adminApi,makeCourse,origin,receiptPng,hydrated,verifyMailbox} from './support';

test('a purchased chapter stays with its buyer across logout, back, and another user',async({page,browser})=>{
 test.setTimeout(180000);
 const secret='ISOLATION-SECRET-TOKEN';
 const admin=await adminApi();
 const course=await makeCourse(admin,secret);
 const email=`owner-${crypto.randomUUID()}@example.test`;
 const password='Browser-test-password-42';
 const owner=await browser.newContext({baseURL:origin,extraHTTPHeaders:{Origin:origin}});
 const registered=await owner.request.post('/api/v1/auth/register',{data:{email,password}});
 expect(registered.ok(),await registered.text()).toBeTruthy();
 await verifyMailbox(email,(path,data)=>owner.request.post(path,{data,headers:{Origin:origin}}));
 const loggedIn=await owner.request.post('/api/v1/auth/login',{data:{email,password}});
 expect(loggedIn.ok(),await loggedIn.text()).toBeTruthy();
 const csrf=((await (await owner.request.get('/api/v1/auth/csrf')).json()).data.csrf_token) as string;
 const created=await owner.request.post('/api/v1/orders',{data:{product_id:course.product.id,quantity:1},headers:{'X-CSRF-Token':csrf,'Idempotency-Key':crypto.randomUUID()}});
 expect(created.ok(),await created.text()).toBeTruthy();
 const order=(await created.json()).data;
 const uploaded=await owner.request.post(`/api/v1/orders/${order.id}/payment-proof`,{multipart:{file:{name:'receipt.png',mimeType:'image/png',buffer:receiptPng}},headers:{'X-CSRF-Token':csrf}});
 expect(uploaded.ok(),await uploaded.text()).toBeTruthy();
 const proof=(await uploaded.json()).data.id as string;
 await admin.write('post','/admin/payments/'+proof+'/approve',{actual_amount_cents:9900,payment_channel:'bank',transaction_reference:'ISO-'+crypto.randomUUID(),receipt_confirmed:true});

 await page.goto('/login?returnTo='+encodeURIComponent('/tutorials/'+course.slug));
 await hydrated(page);
 await page.getByLabel('邮箱').fill(email);
 await page.getByLabel('密码',{exact:true}).fill(password);
 await page.getByRole('button',{name:'登录'}).click();
 await hydrated(page);
 await page.getByRole('button',{name:/02 事务与授权/}).click();
 await expect(page.getByText(secret)).toBeVisible();

 const intruder=await browser.newContext({baseURL:origin,extraHTTPHeaders:{Origin:origin}});
 const intruderEmail=`intruder-${crypto.randomUUID()}@example.test`;
 expect((await intruder.request.post('/api/v1/auth/register',{data:{email:intruderEmail,password}})).ok()).toBeTruthy();
 await verifyMailbox(intruderEmail,(path,data)=>intruder.request.post(path,{data,headers:{Origin:origin}}));
 const [ownerChapter,intruderChapter,ownerHtml,intruderHtml]=await Promise.all([
  owner.request.get(`/api/v1/tutorials/${course.tutorial.id}/chapters/${course.paid.id}`),
  intruder.request.get(`/api/v1/tutorials/${course.tutorial.id}/chapters/${course.paid.id}`),
  owner.request.get('/tutorials/'+course.slug),
  intruder.request.get('/tutorials/'+course.slug),
 ]);
 expect(ownerChapter.status()).toBe(200);
 expect(await ownerChapter.text()).toContain(secret);
 expect(intruderChapter.status()).toBe(403);
 expect(await intruderChapter.text()).not.toContain(secret);
 expect(await ownerHtml.text()).not.toContain(secret);
 expect(await intruderHtml.text()).not.toContain(secret);

 await page.getByRole('button',{name:'退出登录'}).click();
 await expect(page).toHaveURL(origin+'/');
 await page.goBack();
 await expect(page.getByText(secret)).toHaveCount(0);

 await page.goto('/login?returnTo='+encodeURIComponent('/tutorials/'+course.slug));
 await hydrated(page);
 await page.getByLabel('邮箱').fill(intruderEmail);
 await page.getByLabel('密码',{exact:true}).fill(password);
 await page.getByRole('button',{name:'登录'}).click();
 await hydrated(page);
 await page.getByRole('button',{name:/02 事务与授权/}).click();
 await expect(page.getByText('购买后解锁本章节')).toBeVisible();
 await expect(page.getByText(secret)).toHaveCount(0);
 await admin.api.dispose();
 await owner.close();
 await intruder.close();
});
