package de.sve.backend.manager;

import java.util.ArrayList;
import java.util.Base64;
import java.util.Collections;
import java.util.List;
import java.util.stream.Collectors;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import de.sve.backend.mail.Attachment;
import de.sve.backend.mail.Mail;
import de.sve.backend.model.contact.Email;
import de.sve.backend.model.contact.Emails;
import de.sve.backend.model.contact.Message;

public class ContactManager {

	private static final Logger LOG = LoggerFactory.getLogger(ContactManager.class);

	public static void message(Message message) throws Exception {
		Mail.Builder mail = Mail.via(message.type().mailAccount());
		// subject
		mail.subject("[Kontakt@Web] Nachricht von " + message.name()); //$NON-NLS-1$
		// content
		StringBuilder content = new StringBuilder();
		content.append("\nVor- und Nachname: "); //$NON-NLS-1$
		content.append(message.name().strip());
		String email = message.email().strip();
		content.append("\nEmail: "); //$NON-NLS-1$
		content.append(email);
		String phone = message.phone();
		if (phone != null && phone.strip().length() > 0) {
			content.append("\nTelefon: "); //$NON-NLS-1$
			content.append(phone.strip());
		}
		content.append("\nNachricht:\n"); //$NON-NLS-1$
		content.append(message.message().strip());
		mail.content(content.toString())
			.to(message.to())
			.replyTo(email)
			.send();
		LOG.info("Info message has been send successfully"); //$NON-NLS-1$
	}

	public static void emails(Emails emails) throws Exception {
		List<Mail> mails = new ArrayList<>();
		for (Email email : emails.emails()) {
			List<Email.Attachment> attachments = email.attachments();
			if (attachments == null) {
				attachments = Collections.emptyList();
			}
			mails.add(Mail.via(email.type().mailAccount())
						  .to(email.to())
						  .subject(email.subject())
						  .content(email.content())
						  .attachments(attachments.stream()
								  				  .map(a -> Attachment.create(a.name(),
								  						  					  Base64.getDecoder().decode(a.data()),
								  						  					  a.mimeType()))
								  				  .collect(Collectors.toList()))
						  .build());
		}
		Mail.send(mails);
		LOG.info("Emails (" + mails.size() + ") has been send successfully"); //$NON-NLS-1$ //$NON-NLS-2$
	}

}
