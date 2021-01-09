package de.sve.backend.mail;

import java.util.List;

import javax.annotation.Nullable;

import com.google.auto.value.AutoValue;
import com.google.common.collect.ImmutableList;
import com.google.common.collect.ImmutableSet;

import jakarta.mail.MessagingException;

/**
 * Create and send an email.
 *
 * @author mseele
 */
@AutoValue
public abstract class Mail {

	public static Builder via(MailAccount account) {
		return new AutoValue_Mail.Builder().sender(account)
										   .bcc()
										   .attachments();
	}

	public static void send(List<Mail> mails) throws MessagingException {
		Postman.deliver(mails);
	}

	public abstract MailAccount sender();

	public abstract ImmutableSet<String> to();

	public abstract ImmutableSet<String> bcc();

	@Nullable
	public abstract String replyTo();

	public abstract String subject();

	public abstract String content();

	public abstract ImmutableList<Attachment> attachments();

	@AutoValue.Builder
	public abstract static class Builder {

		abstract Builder sender(MailAccount value);

		public abstract Builder to(String... value);

		public abstract Builder bcc(String... value);

		public abstract Builder replyTo(String value);

		public abstract Builder subject(String value);

		public abstract Builder content(String value);

		public abstract Builder attachments(List<Attachment> value);

		public abstract Builder attachments(Attachment... value);

		public abstract Mail build();

		public void send() throws MessagingException {
			Postman.deliver(build());
		}

	}

}
